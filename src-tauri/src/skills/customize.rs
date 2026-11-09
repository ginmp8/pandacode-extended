//! ZIP acquisition and read-only previews for the standalone Customize workspace.
//! The existing Library performs the only final atomic installation.
use std::{collections::HashSet, fs, io::Read, path::PathBuf};

use base64::Engine;
use serde::Serialize;
use tauri::AppHandle;

use super::{frontmatter, github, library, slug, valid_id, Kind, Source, Item, MAX_BYTES, MAX_FILES};

const MAX_ARCHIVE_BYTES: u64 = 32 << 20;
const MAX_ARCHIVE_ENTRIES: usize = 512;
const MAX_PREVIEW_BYTES: usize = 512 << 10;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZipInstall {
    pub item: Item,
    pub replaced: bool,
    pub previous_source: Option<Source>,
    pub impacted_harnesses: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFile {
    pub path: String,
    pub text: Option<String>,
    pub size: usize,
    pub binary: bool,
}

fn valid_portable_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return false;
    }
    path.split('/').all(|part| {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "COM1" | "COM2" | "COM3" | "COM4" | "COM5" | "COM6" | "COM7" | "COM8" | "COM9" | "LPT1" | "LPT2" | "LPT3" | "LPT4" | "LPT5" | "LPT6" | "LPT7" | "LPT8" | "LPT9");
        !part.is_empty() && part != "." && part != ".." && part != ".git"
            && !part.ends_with([' ', '.']) && !reserved && !part.contains(':')
            && !part.chars().any(char::is_control)
    })
}

/// Validate bounded YAML flow syntax and reject incomplete nested collections.
fn valid_yaml_flow(value: &str) -> bool {
    let mut stack = Vec::new();
    let mut quote: Option<char> = None;
    let mut escape = false;
    let mut finished = false;
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(delimiter) = quote {
            if delimiter == '"' {
                if escape { escape = false; }
                else if c == '\\' { escape = true; }
                else if c == '"' { quote = None; }
            } else if c == '\'' {
                if chars.peek() == Some(&'\'') { chars.next(); }
                else { quote = None; }
            }
            continue;
        }
        if finished {
            if !c.is_whitespace() { return false; }
            continue;
        }
        match c {
            '"' | '\'' => quote = Some(c),
            '[' | '{' => stack.push(c),
            ']' => { if stack.pop() != Some('[') { return false; } },
            '}' => { if stack.pop() != Some('{') { return false; } },
            _ => {}
        }
        if matches!(c, ']' | '}') && stack.is_empty() { finished = true; }
    }
    finished && stack.is_empty() && quote.is_none() && !escape
}

fn valid_yaml_value(key: &str, value: &str) -> bool {
    let forbidden_required = matches!(key, "name" | "description")
        && (value.starts_with(['[', '{', '!', '&', '*'])
            || matches!(value, "~" | "null" | "Null" | "NULL"));
    let malformed_double = value.starts_with('"') && serde_json::from_str::<String>(value).is_err();
    let malformed_single = value.starts_with('\'')
        && (value.len() < 2 || !value.ends_with('\'')
            || value[1..value.len() - 1].replace("''", "").contains('\''));
    !forbidden_required && !malformed_double && !malformed_single
        && !(value.starts_with(['[', '{']) && !valid_yaml_flow(value))
        && value.starts_with('"') == value.ends_with('"')
        && value.starts_with('\'') == value.ends_with('\'')
        && (value.starts_with(['"', '\'', '[', '{']) || !value.contains(": "))
}

/// Validate a bounded, declarative YAML frontmatter shape supported by the
/// native Skills Library, including optional nested maps/lists and flow values.
fn validate_zip_frontmatter(markdown: &str) -> Result<String, String> {
    let normalized = markdown.replace("\r\n", "\n");
    let body = normalized.strip_prefix("---\n")
        .ok_or("SKILL.md must start with YAML frontmatter.")?;
    let end = body.lines().position(|line| line == "---")
        .ok_or("SKILL.md YAML frontmatter must have a closing delimiter.")?;
    let mut name_seen = false;
    let mut description_seen = false;
    let mut block_scalar = false;
    let mut nested = false;
    for line in body.lines().take(end) {
        if line.trim().is_empty() || line.trim_start().starts_with('#') { continue; }
        let indentation = line.bytes().take_while(|b| *b == b' ').count();
        if line.starts_with('\t') || line[indentation..].starts_with('\t') {
            return Err("SKILL.md YAML frontmatter has invalid tab indentation.".into());
        }
        if indentation > 0 {
            if block_scalar { continue; }
            if !nested {
                return Err("SKILL.md YAML frontmatter has unexpected indentation.".into());
            }
            let child = line.trim();
            let field = child.strip_prefix("- ").unwrap_or(child);
            if field.is_empty() { continue; }
            if let Some((key, value)) = field.split_once(':') {
                let value = value.trim();
                if key.is_empty() || (!value.is_empty() && !valid_yaml_value(key, value)) {
                    return Err("SKILL.md YAML metadata has a malformed nested field.".into());
                }
            } else if !valid_yaml_value("", field) {
                return Err("SKILL.md YAML metadata has a malformed nested value.".into());
            }
            continue;
        }
        let (key, value) = line.split_once(':')
            .ok_or("SKILL.md YAML frontmatter contains a malformed field.")?;
        if key.is_empty() || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
            return Err("SKILL.md YAML frontmatter contains an invalid key.".into());
        }
        if key == "name" {
            if name_seen { return Err("SKILL.md has duplicate name declarations.".into()); }
            name_seen = true;
        } else if key == "description" {
            if description_seen { return Err("SKILL.md has duplicate description declarations.".into()); }
            description_seen = true;
        }
        let value = value.trim();
        block_scalar = matches!(value, "|" | ">" | "|-" | ">-" | "|+" | ">+");
        nested = value.is_empty();
        if !block_scalar && !nested && !valid_yaml_value(key, value) {
            return Err("SKILL.md YAML frontmatter has a malformed scalar or collection.".into());
        }
    }
    let (name, description, _) = frontmatter(&normalized);
    let name = name.ok_or("SKILL.md must declare a name in YAML frontmatter.")?;
    if !name_seen || !description_seen || description.is_none_or(|text| text.is_empty())
        || !valid_id(&slug(&name)) {
        return Err("SKILL.md must declare a valid name and description in YAML frontmatter.".into());
    }
    Ok(name)
}

fn read_zip(path: &std::path::Path) -> Result<super::Files, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("Could not open ZIP: {e}"))?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_ARCHIVE_BYTES {
        return Err("Choose a regular ZIP archive smaller than 32 MiB.".into());
    }
    let reader = fs::File::open(path).map_err(|e| format!("Could not open ZIP: {e}"))?;
    let mut archive = zip::ZipArchive::new(reader).map_err(|_| "The selected file is not a valid ZIP archive.")?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("ZIP contains too many entries.".into());
    }
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut names = HashSet::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| "Cannot inspect a ZIP entry.")?;
        let entry_name = entry.name().map_err(|_| "Invalid ZIP entry name.")?.into_owned();
        let path = entry_name.trim_end_matches('/');
        if !valid_portable_path(path) {
            return Err(format!("ZIP contains an unsafe path: {path}."));
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if kind != 0 && kind != 0o100000 && kind != 0o040000 {
                return Err(format!("ZIP contains a link or special file: {path}."));
            }
        }
        if !names.insert(path.to_ascii_lowercase()) {
            return Err(format!("ZIP contains duplicate or case-conflicting paths: {path}."));
        }
        if entry.is_dir() { continue; }
        if files.len() >= MAX_FILES || entry.size() > MAX_BYTES as u64
            || entry.size() > entry.compressed_size().max(1).saturating_mul(1000) {
            return Err("ZIP exceeds package file, size or compression-ratio limits.".into());
        }
        total = total.checked_add(entry.size()).ok_or("ZIP package size overflow.")?;
        if total > MAX_BYTES as u64 {
            return Err("ZIP uncompressed content exceeds 24 MiB.".into());
        }
        let expected_size = entry.size();
        let mut content = Vec::new();
        (&mut entry).take(MAX_BYTES as u64 + 1).read_to_end(&mut content)
            .map_err(|_| "Unable to decompress ZIP entry.")?;
        if content.len() > MAX_BYTES || content.len() as u64 != expected_size {
            return Err("ZIP entry length does not match the archive metadata.".into());
        }
        files.push((path.to_string(), content));
    }
    let skill_roots: Vec<String> = files.iter().filter_map(|(name, _)| {
        if name == "SKILL.md" { Some(String::new()) }
        else { name.strip_suffix("/SKILL.md").map(str::to_string) }
    }).collect();
    if skill_roots.len() != 1 {
        return Err("ZIP must contain exactly one skill package with SKILL.md.".into());
    }
    let wrapper = &skill_roots[0];
    if !wrapper.is_empty() && wrapper.contains('/') {
        return Err("ZIP may contain at most one outer wrapper directory.".into());
    }
    if !wrapper.is_empty() {
        let prefix = format!("{wrapper}/");
        if files.iter().any(|(name, _)| !name.starts_with(&prefix)) {
            return Err("ZIP has files outside the single skill folder.".into());
        }
        for (name, _) in &mut files {
            *name = name.strip_prefix(&prefix).unwrap_or(name).to_string();
        }
    }
    let main = files.iter().find(|(name, _)| name == "SKILL.md")
        .ok_or("The package has no SKILL.md.")?;
    let markdown = std::str::from_utf8(&main.1).map_err(|_| "SKILL.md must be UTF-8.")?;
    validate_zip_frontmatter(markdown)?;
    Ok(files)
}

pub fn import_zip(app: &AppHandle, path: &str) -> Result<ZipInstall, String> {
    import_zip_progress(app, path, false, |_| {})
}

pub fn import_zip_progress(app: &AppHandle, path: &str, allow_replace: bool, mut progress: impl FnMut(&str)) -> Result<ZipInstall, String> {
    let path = PathBuf::from(path);
    if !path.is_absolute() || path.extension().and_then(|ext| ext.to_str()).is_none_or(|ext| !ext.eq_ignore_ascii_case("zip")) {
        return Err("Choose an absolute path to a .zip skill package.".into());
    }
    progress("validating");
    let files = read_zip(&path)?;
    let main = files.iter().find(|(name, _)| name == "SKILL.md").ok_or("Missing SKILL.md.")?;
    let content = std::str::from_utf8(&main.1).map_err(|_| "Invalid SKILL.md.")?;
    let name = validate_zip_frontmatter(content)?;
    let id = slug(&name);
    let library = library(app)?;
    let previous = library.list()?.into_iter().find(|entry| entry.meta.id == id);
    if let Some(existing) = previous.as_ref().filter(|_| !allow_replace) {
        return Err(format!(
            "ZIP replacement requires confirmation: {name} is already installed from {} ({} harness activations). Installing this ZIP would replace the central library package but not native harness files.",
            existing.meta.source.location, existing.meta.activations.len()
        ));
    }
    let previous_source = previous.as_ref().map(|item| item.meta.source.clone());
    let impacted_harnesses = previous.as_ref().map(|item| item.meta.activations.iter()
        .map(|activation| activation.harness.clone()).collect()).unwrap_or_default();
    let location = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    progress("installing");
    let item = library.import_as_guarded(Kind::Skill, files, &name, Source {
        kind: "zip".into(), location, git_ref: None, commit: None,
    }, false, !allow_replace)?;
    progress("installed");
    Ok(ZipInstall { item, replaced: previous.is_some(), previous_source, impacted_harnesses })
}

fn preview_file(path: &str, bytes: Vec<u8>) -> PreviewFile {
    let size = bytes.len();
    let text = if size <= MAX_PREVIEW_BYTES { String::from_utf8(bytes).ok() } else { None };
    let binary = text.is_none();
    PreviewFile { path: path.to_string(), text, size, binary }
}

pub fn installed_preview(app: &AppHandle, id: &str, file: &str) -> Result<PreviewFile, String> {
    if !valid_id(id) || !valid_portable_path(file) {
        return Err("Choose a valid skill and package file.".into());
    }
    let files = library(app)?.files(id)?;
    let (_, bytes) = files.into_iter().find(|(path, _)| path == file)
        .ok_or("The requested file is not in the installed skill.")?;
    Ok(preview_file(file, bytes))
}

pub fn github_preview(repo: &str, package: &str, commit: &str, file: &str) -> Result<PreviewFile, String> {
    if !github::valid_repo(repo) || !valid_portable_path(file)
        || (!package.is_empty() && !github::valid_path(package))
        || commit.len() != 40 || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Choose a previously scanned GitHub skill and full commit SHA.".into());
    }
    let path = if package.is_empty() { file.to_string() } else { format!("{package}/{file}") };
    if !github::valid_path(&path) {
        return Err("The preview file is not a safe relative path.".into());
    }
    let value = github::api(&format!("repos/{repo}/contents/{path}?ref={commit}"))
        .map_err(|e| super::catalog::explain(&e))?;
    if value["type"] != "file" || value["size"].as_u64().unwrap_or(u64::MAX) > MAX_PREVIEW_BYTES as u64 {
        return Err("Only text files smaller than 512 KiB can be previewed.".into());
    }
    let encoded = value["content"].as_str().ok_or("GitHub did not supply the requested file.")?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(encoded.replace(['\n', '\r'], ""))
        .map_err(|_| "GitHub returned unreadable file content.")?;
    Ok(preview_file(file, bytes))
}

#[cfg(test)]
mod tests {
    use super::valid_portable_path;
    #[test]
    fn zip_packages_are_validated_before_any_library_installation() {
        use super::read_zip;
        use std::io::Write;
        fn archive(entries: &[(&str, &str)]) -> std::path::PathBuf {
            let path = std::env::temp_dir().join(format!("pandacode-zip-{}.zip", uuid::Uuid::new_v4()));
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            for (name, body) in entries {
                writer.start_file((*name).to_string(), zip::write::SimpleFileOptions::default()).unwrap();
                writer.write_all(body.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
            path
        }
        let good = archive(&[
            ("wrapper/SKILL.md", "---\nname: Valid\ndescription: Safe package\n---\n# Valid"),
            ("wrapper/references/guide.md", "Help"),
        ]);
        let files = read_zip(&good).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|(path,_)| path=="SKILL.md"));
        let _ = std::fs::remove_file(good);
        let multiple = archive(&[
            ("one/SKILL.md", "---\nname: One\ndescription: First\n---"),
            ("two/SKILL.md", "---\nname: Two\ndescription: Second\n---"),
        ]);
        assert!(read_zip(&multiple).unwrap_err().contains("exactly one"));
        let _ = std::fs::remove_file(multiple);
        let traversal = archive(&[("../SKILL.md", "malicious")]);
        assert!(read_zip(&traversal).unwrap_err().contains("unsafe path"));
        let _ = std::fs::remove_file(traversal);
    }

    #[test]
    fn rejects_malformed_zip_frontmatter() {
        use super::validate_zip_frontmatter;
        assert!(validate_zip_frontmatter("---\\nname: Good\\ndescription: Good description\\n---\\n# Good".replace("\\n", "\n").as_str()).is_ok());
        assert!(validate_zip_frontmatter("---\\nname: Broken\\ndescription: \"unterminated\\n---".replace("\\n", "\n").as_str()).is_err());
        assert!(validate_zip_frontmatter("---\\nname: One\\nname: Two\\ndescription: Duplicate\\n---".replace("\\n", "\n").as_str()).is_err());
        assert!(validate_zip_frontmatter("---\\nname: Missing\\nnot-a-field\\ndescription: Test\\n---".replace("\\n", "\n").as_str()).is_err());
        assert!(validate_zip_frontmatter("---\nname: [unclosed\ndescription: Test\n---").is_err());
        assert!(validate_zip_frontmatter("---\nname: Valid\ndescription: {invalid-flow\n---").is_err());
        assert!(validate_zip_frontmatter("---\nname: \"invalid\\q\"\ndescription: Test\n---").is_err());
        assert!(validate_zip_frontmatter("---\nname: Valid\ndescription: >-\n  Safe multi-line\n  description.\n---").is_ok());
        assert!(validate_zip_frontmatter("---\nname: Valid\ndescription: Good\nmetadata:\n  author: Alice\n  labels:\n    - tooling\n---").is_ok());
        assert!(validate_zip_frontmatter("---\nname: Valid\ndescription: Good\nmetadata: [missing\n---").is_err());
        assert!(validate_zip_frontmatter("---\nname: Valid\ndescription: Good\nmetadata:\n  labels: [missing\n---").is_err());
    }
    #[test]
    fn rejects_unsafe_zip_paths() {
        for path in ["../SKILL.md", "/SKILL.md", "a\\SKILL.md", "a/../b", "con.txt", "folder/COM1.md", ".git/config", "file."] {
            assert!(!valid_portable_path(path), "{path}");
        }
        assert!(valid_portable_path("references/guide.md"));
        assert!(valid_portable_path(".github/settings.md"));
        assert!(valid_portable_path(".config/reference.md"));
        assert!(valid_portable_path("SKILL.md"));
    }
}
