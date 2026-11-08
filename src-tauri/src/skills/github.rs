//! Import a skill or rule from a GitHub repository path through the user's `gh` login.
use base64::Engine;
use std::time::Duration;

use super::{Files, MAX_BYTES, MAX_FILES};

const TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let ok = |part: Option<&str>| part.is_some_and(|p| !p.is_empty() && !p.starts_with(['.', '-']) && p.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)));
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}
pub(super) fn valid_path(path: &str) -> bool {
    path.split('/').all(|part| !part.is_empty() && part != "." && part != ".." && part.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_. ".contains(&b)))
}
pub(super) fn valid_ref(git_ref: &str) -> bool {
    !git_ref.is_empty() && !git_ref.starts_with('-') && !git_ref.contains("..") && git_ref.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_./".contains(&b))
}

pub(super) fn api(endpoint: &str) -> Result<serde_json::Value, String> {
    let home = crate::agent_usage_providers::home().unwrap_or_else(std::env::temp_dir);
    let capture = crate::github::run_gh(&home, &["api", endpoint], TIMEOUT)?;
    crate::github::json(&capture)
}

/// Parse a GitHub URL or explicit owner/repo + ref + directory without guessing which
/// segments of a slash-containing branch are part of its path. Full SHA URLs are unambiguous.
pub(super) fn selection(input: &str, path: &str, git_ref: Option<&str>) -> Result<(String, String, Option<String>), String> {
    let input = input.trim().trim_end_matches('/');
    let explicit_path = path.trim_matches('/');
    let explicit_ref = git_ref.map(str::trim).filter(|r| !r.is_empty());
    let repository: String;
    let mut resolved_path = String::new();
    let mut resolved_ref = explicit_ref.map(str::to_string);
    if let Some(url) = input.strip_prefix("https://github.com/") {
        if url.contains('?') || url.contains('#') {
            return Err("Remove query strings and fragments from the GitHub URL.".into());
        }
        let parts: Vec<&str> = url.split('/').collect();
        if parts.len() < 2 {
            return Err("Enter a GitHub repository URL with an owner and name.".into());
        }
        repository = format!("{}/{}", parts[0], parts[1].trim_end_matches(".git"));
        if parts.len() > 2 {
            if !matches!(parts[2], "tree" | "blob") || parts.len() < 4 {
                return Err("Use a repository URL or its /tree/ref/path or /blob/ref/SKILL.md URL.".into());
            }
            let tail = parts[3..].join("/");
            let reference = match explicit_ref {
                Some(reference) => reference.to_string(),
                None if parts[3].len() == 40 && parts[3].bytes().all(|b| b.is_ascii_hexdigit()) => parts[3].to_string(),
                None if matches!(parts[3], "main" | "master") => parts[3].to_string(),
                None => return Err("For a tree URL with a named branch or tag, enter the exact ref separately; branch names can contain slashes.".into()),
            };
            resolved_path = if tail == reference { String::new() } else {
                tail.strip_prefix(&format!("{reference}/"))
                    .ok_or("The GitHub URL does not match the explicit ref.")?
                    .to_string()
            };
            resolved_ref = Some(reference);
            if parts[2] == "blob" && !resolved_path.ends_with("/SKILL.md") && resolved_path != "SKILL.md" {
                return Err("A skill file URL must end in SKILL.md.".into());
            }
        }
    } else if input.starts_with("http://") || input.starts_with("https://") {
        return Err("Only https://github.com repository URLs are supported.".into());
    } else {
        repository = input.trim_end_matches(".git").to_string();
    }
    if !explicit_path.is_empty() {
        if !resolved_path.is_empty() && explicit_path != resolved_path && format!("{explicit_path}/SKILL.md") != resolved_path {
            return Err("The explicit path does not match the GitHub URL path.".into());
        }
        resolved_path = explicit_path.to_string();
    }
    if let Some(dir) = resolved_path.strip_suffix("/SKILL.md") {
        resolved_path = dir.to_string();
    } else if resolved_path == "SKILL.md" {
        resolved_path.clear();
    }
    if !valid_repo(&repository) || (!resolved_path.is_empty() && !valid_path(&resolved_path))
        || resolved_ref.as_deref().is_some_and(|r| !valid_ref(r)) {
        return Err("Enter a valid owner/name, path and ref.".into());
    }
    Ok((repository, resolved_path, resolved_ref))
}

/// `owner/name`, a path inside it (empty for the root), and an optional branch, tag or commit.
/// Returns the files under that path (relative to it), and the commit they came from.
pub fn fetch(repo: &str, path: &str, git_ref: Option<&str>) -> Result<(Files, String), String> {
    fetch_using(repo, path, git_ref, &api)
}

pub(super) fn fetch_using(repo: &str, path: &str, git_ref: Option<&str>, api: super::catalog::Api<'_>) -> Result<(Files, String), String> {
    let path = path.trim_matches('/');
    if !valid_repo(repo) {
        return Err("Enter the repository as owner/name.".into());
    }
    if !path.is_empty() && !valid_path(path) {
        return Err("That repository path is not valid.".into());
    }
    let git_ref = git_ref.map(str::trim).filter(|r| !r.is_empty());
    if git_ref.is_some_and(|r| !valid_ref(r)) {
        return Err("That branch, tag or commit is not valid.".into());
    }
    let commit = api(&format!("repos/{repo}/commits/{}", git_ref.unwrap_or("HEAD")))?["sha"]
        .as_str()
        .ok_or("GitHub did not return a commit for that ref.")?
        .to_string();
    let mut files = Vec::new();
    let mut total = 0;
    let mut pending = vec![path.to_string()];
    while let Some(current) = pending.pop() {
        let value = api(&format!("repos/{repo}/contents/{current}?ref={commit}"))?;
        let entries = match value {
            serde_json::Value::Array(entries) => entries,
            single => vec![single],
        };
        // A nested SKILL.md starts a distinct package: do not download or copy
        // any of its files into a selected parent skill (including root skills).
        if current != path && entries.iter().any(|entry| entry["type"] == "file" && entry["name"] == "SKILL.md") {
            continue;
        }
        for entry in entries {
            let entry_path = entry["path"].as_str().unwrap_or_default().to_string();
            let name = entry["name"].as_str().unwrap_or_default();
            if name.starts_with('.') || !valid_path(&entry_path) {
                continue;
            }
            match entry["type"].as_str() {
                Some("dir") => pending.push(entry_path),
                Some("file") => {
                    if files.len() >= MAX_FILES {
                        return Err(format!("That path holds more than {MAX_FILES} files."));
                    }
                    let declared_size = entry["size"].as_u64().unwrap_or(0) as usize;
                    if declared_size > MAX_BYTES || declared_size > MAX_BYTES.saturating_sub(total) {
                        return Err(format!("That path is larger than {} MiB.", MAX_BYTES >> 20));
                    }
                    // Directory listing results usually omit content. Request the immutable Git
                    // blob rather than trusting a truncated Contents response (which may return
                    // encoding=none for large files), and fail closed on unsupported encoding.
                    let detail = if entry["encoding"] == "base64" && entry["content"].is_string() {
                        entry.clone()
                    } else {
                        let sha = entry["sha"].as_str().ok_or("GitHub did not return the file object ID.")?;
                        if !sha.bytes().all(|b| b.is_ascii_hexdigit()) || sha.len() != 40 {
                            return Err(format!("Invalid file object ID for {entry_path}."));
                        }
                        api(&format!("repos/{repo}/git/blobs/{sha}"))?
                    };
                    if detail["encoding"] != "base64" {
                        return Err(format!("GitHub did not provide Base64 file contents for {entry_path}."));
                    }
                    let content = detail["content"].as_str().ok_or_else(|| format!("Missing file contents for {entry_path}."))?;
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(content.replace(['\n', '\r'], ""))
                        .map_err(|_| format!("GitHub returned unreadable content for {entry_path}."))?;
                    if declared_size != 0 && declared_size != bytes.len() {
                        return Err(format!("GitHub file size mismatch for {entry_path}."));
                    }
                    total = total.checked_add(bytes.len()).ok_or("GitHub file size overflow.")?;
                    if total > MAX_BYTES {
                        return Err(format!("That path is larger than {} MiB.", MAX_BYTES >> 20));
                    }
                    // A single-file import keeps its own name; a folder keeps its layout.
                    let relative = if entry_path == path { name.to_string() } else { entry_path[path.len()..].trim_start_matches('/').to_string() };
                    files.push((relative, bytes));
                }
                _ => {}
            }
        }
    }
    if files.is_empty() {
        return Err("Nothing to import at that path.".into());
    }
    Ok((files, commit))
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn a_parent_skill_does_not_fetch_or_install_nested_skill_packages() {
        use std::cell::RefCell;
        let requests = RefCell::new(Vec::<String>::new());
        let api = |endpoint: &str| -> Result<serde_json::Value, String> {
            requests.borrow_mut().push(endpoint.to_string());
            match endpoint {
                "repos/me/kit/commits/HEAD" => Ok(serde_json::json!({"sha":"abc"})),
                "repos/me/kit/contents/?ref=abc" => Ok(serde_json::json!([
                    {"path":"SKILL.md","name":"SKILL.md","type":"file","size":4,"encoding":"base64","content":"dGVzdA=="},
                    {"path":"child","name":"child","type":"dir"}
                ])),
                "repos/me/kit/contents/child?ref=abc" => Ok(serde_json::json!([
                    {"path":"child/SKILL.md","name":"SKILL.md","type":"file","size":4,"encoding":"base64","content":"dGVzdA=="},
                    {"path":"child/guide.md","name":"guide.md","type":"file","size":4,"encoding":"base64","content":"dGVzdA=="}
                ])),
                _ => Err(format!("Unexpected GitHub endpoint: {endpoint}")),
            }
        };
        let (files, commit) = fetch_using("me/kit", "", None, &api).unwrap();
        assert_eq!(commit, "abc");
        assert_eq!(files.len(), 1, "nested SKILL.md must remain a separate package");
        assert_eq!(files[0].0, "SKILL.md");
        assert_eq!(requests.borrow().len(), 3, "nested package files must not be downloaded");
    }

    #[test]
    fn validates_repository_input() {
        assert!(valid_repo("anthropics/skills"));
        assert!(!valid_repo("anthropics"));
        assert!(!valid_repo("-x/skills"));
        assert!(!valid_repo("a/b/c"));
        assert!(valid_path("skills/pdf"));
        assert!(!valid_path("skills/../secrets"));
        assert!(valid_ref("v1.2.0") && valid_ref("feature/x"));
        assert!(!valid_ref("--output=x") && !valid_ref("a..b"));
        assert_eq!(
            selection("https://github.com/ginmp8/rhapsodia/tree/main/skills/archify", "", None).unwrap(),
            ("ginmp8/rhapsodia".into(), "skills/archify".into(), Some("main".into()))
        );
        let sha = "a".repeat(40);
        assert_eq!(
            selection(&format!("https://github.com/o/r/blob/{sha}/skills/pdf/SKILL.md"), "", None).unwrap().2,
            Some(sha)
        );
        assert_eq!(
            selection("https://github.com/o/r/tree/feature/a/skills/pdf", "", Some("feature/a")).unwrap().1,
            "skills/pdf"
        );
        assert!(selection("https://github.com/o/r/tree/feature/a/skills/pdf", "", None).is_err());
        assert!(selection("https://github.com/o/r/tree/main/skills/pdf", "skills/other", None).is_err());
    }
}
