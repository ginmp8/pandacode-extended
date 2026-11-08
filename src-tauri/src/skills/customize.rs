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
        !part.is_empty() && part != "." && part != ".." && !part.starts_with('.')
            && !part.ends_with([' ', '.']) && !reserved && !part.contains(':')
            && !part.chars().any(char::is_control)
    })
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
        let path = entry.name().trim_end_matches('/');
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
    let (name, description, _) = frontmatter(markdown);
    if name.as_deref().is_none_or(str::is_empty) || description.as_deref().is_none_or(str::is_empty)
        || !name.as_deref().is_some_and(|name| valid_id(&slug(name))) {
        return Err("SKILL.md must declare a valid name and description in YAML frontmatter.".into());
    }
    Ok(files)
}

pub fn import_zip(app: &AppHandle, path: &str) -> Result<ZipInstall, String> {
    import_zip_progress(app, path, |_| {})
}

pub fn import_zip_progress(app: &AppHandle, path: &str, mut progress: impl FnMut(&str)) -> Result<ZipInstall, String> {
    let path = PathBuf::from(path);
    if !path.is_absolute() || path.extension().and_then(|ext| ext.to_str()).is_none_or(|ext| !ext.eq_ignore_ascii_case("zip")) {
        return Err("Choose an absolute path to a .zip skill package.".into());
    }
    progress("validating");
    let files = read_zip(&path)?;
    let main = files.iter().find(|(name, _)| name == "SKILL.md").ok_or("Missing SKILL.md.")?;
    let content = std::str::from_utf8(&main.1).map_err(|_| "Invalid SKILL.md.")?;
    let name = frontmatter(content).0.ok_or("Missing skill name.")?;
    let id = slug(&name);
    let library = library(app)?;
    let previous = library.list()?.into_iter().find(|entry| entry.meta.id == id);
    let previous_source = previous.as_ref().map(|item| item.meta.source.clone());
    let impacted_harnesses = previous.as_ref().map(|item| item.meta.activations.iter()
        .map(|activation| activation.harness.clone()).collect()).unwrap_or_default();
    let location = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
    progress("installing");
    let item = library.import(Kind::Skill, files, &name, Source {
        kind: "zip".into(), location, git_ref: None, commit: None,
    })?;
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
    fn rejects_unsafe_zip_paths() {
        for path in ["../SKILL.md", "/SKILL.md", "a\\SKILL.md", "a/../b", "con.txt", "folder/COM1.md", ".hidden/SKILL.md", "file."] {
            assert!(!valid_portable_path(path), "{path}");
        }
        assert!(valid_portable_path("references/guide.md"));
        assert!(valid_portable_path("SKILL.md"));
    }
}
