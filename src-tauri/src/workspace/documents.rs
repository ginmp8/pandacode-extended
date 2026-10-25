//! Workspace-only document bytes. Archive limits count actual decompressed bytes,
//! not just the sizes an untrusted central directory claims.
use super::scoped_path;
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Cursor, Read},
    path::Path,
};

const FILE_BYTES: u64 = 64 * 1024 * 1024;
const ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const ARCHIVE_ENTRIES: usize = 2048;

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn supported(path: &Path) -> bool {
    matches!(
        extension(path).as_str(),
        "pdf"
            | "docx"
            | "xlsx"
            | "xlsm"
            | "xls"
            | "ods"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "avif"
            | "bmp"
            | "ico"
            | "svg"
    )
}

fn protected(path: &Path) -> bool {
    path.components().any(|part| {
        part.as_os_str()
            .to_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(".git"))
    })
}

pub fn read_document_bytes(root: &Path, path: &str) -> Result<Vec<u8>, String> {
    let target = scoped_path(root, path)?;
    let resolved = fs::canonicalize(&target).map_err(|error| error.to_string())?;
    let base = fs::canonicalize(root).map_err(|error| error.to_string())?;
    let relative = resolved
        .strip_prefix(&base)
        .map_err(|_| "Path must stay inside the project")?;
    if !supported(&target) || !supported(&resolved) || protected(relative) {
        return Err("Only PDF, Word (.docx), spreadsheet and image files can be shown".into());
    }
    // Inspect before opening as well, so a FIFO/device cannot make the reader wait.
    let metadata = fs::metadata(&resolved).map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > FILE_BYTES {
        return Err("The file is missing or larger than 64 MiB".into());
    }
    let file = open_document(&base, relative)?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() > FILE_BYTES {
        return Err("The file is missing or larger than 64 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take(FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > FILE_BYTES {
        return Err("The file exceeds the 64 MiB preview limit".into());
    }
    if matches!(
        extension(&resolved).as_str(),
        "docx" | "xlsx" | "xlsm" | "ods"
    ) {
        validate_archive(&bytes)?;
    } else if matches!(
        extension(&resolved).as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" | "ico" | "svg"
    ) {
        validate_image(&bytes, &extension(&resolved))?;
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_document(root: &Path, relative: &Path) -> Result<File, String> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::{ffi::OsStrExt, fs::OpenOptionsExt},
        },
    };
    let mut parent = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(root)
        .map_err(|error| error.to_string())?;
    let parts: Vec<_> = relative.components().collect();
    for (index, component) in parts.iter().enumerate() {
        let name =
            CString::new(component.as_os_str().as_bytes()).map_err(|_| "Invalid document path")?;
        let directory = index + 1 < parts.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if directory { libc::O_DIRECTORY } else { 0 };
        // Each parent is an already-open directory. Renaming or replacing a path
        // while the read starts cannot redirect traversal through a new symlink.
        let fd = unsafe { libc::openat(parent.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        parent = unsafe { File::from_raw_fd(fd) };
    }
    Ok(parent)
}

#[cfg(any(windows, test))]
fn validate_opened_path(root: &Path, relative: &Path, actual: &Path) -> Result<(), String> {
    // The handle, not another lookup of the original pathname, identifies the
    // object that will be read. A renamed object also invalidates this request.
    if actual != root.join(relative) || protected(relative) || !supported(actual) {
        return Err("The opened document no longer matches its workspace path".into());
    }
    Ok(())
}

#[cfg(windows)]
fn open_document(root: &Path, relative: &Path) -> Result<File, String> {
    use std::os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    };
    use windows::Win32::{
        Foundation::HANDLE,
        Storage::FileSystem::{
            GetFinalPathNameByHandleW, FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED,
        },
    };
    // Do not admit alternate data streams, absolute paths or parent components.
    if !relative.components().all(|part| {
        matches!(part, std::path::Component::Normal(name) if !name.to_string_lossy().contains(':'))
    }) {
        return Err("Invalid document path".into());
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0 | FILE_FLAG_BACKUP_SEMANTICS.0)
        .open(root.join(relative))
        .map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err("The document must be a regular file".into());
    }
    // OPEN_REPARSE_POINT protects the final component only. An ancestor can be
    // replaced by a junction, so validate the opened handle before reading bytes.
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            HANDLE(file.as_raw_handle()),
            &mut buffer,
            FILE_NAME_NORMALIZED,
        )
    };
    if length == 0 || length as usize >= buffer.len() {
        return Err("Could not verify the opened document path".into());
    }
    let actual = std::path::PathBuf::from(
        String::from_utf16(&buffer[..length as usize])
            .map_err(|_| "The opened document path is invalid")?,
    );
    validate_opened_path(root, relative, &actual)?;
    Ok(file)
}

#[cfg(not(any(unix, windows)))]
fn open_document(_root: &Path, _relative: &Path) -> Result<File, String> {
    Err("Safe document reads are not supported on this platform".into())
}

pub(super) fn validate_image(bytes: &[u8], kind: &str) -> Result<(), String> {
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Image previews are limited to 8 MiB".into());
    }
    let bad = || "The image has invalid or oversized dimensions".to_owned();
    let bounded = |width: u64, height: u64| -> Result<(), String> {
        if width == 0 || height == 0 || width > 32_000_000 / height {
            Err(bad())
        } else {
            Ok(())
        }
    };
    match kind {
        "svg" => Ok(()), // Sanitized as static SVG with bounded dimensions before browser decoding.
        "gif" if bytes.len() >= 10 && matches!(&bytes[..6], b"GIF87a" | b"GIF89a") => bounded(
            u16::from_le_bytes([bytes[6], bytes[7]]) as u64,
            u16::from_le_bytes([bytes[8], bytes[9]]) as u64,
        ),
        "bmp" if bytes.len() >= 26 && bytes.starts_with(b"BM") => {
            let width = i32::from_le_bytes(bytes[18..22].try_into().unwrap()).unsigned_abs() as u64;
            let height =
                i32::from_le_bytes(bytes[22..26].try_into().unwrap()).unsigned_abs() as u64;
            bounded(width, height)
        }
        "ico" if bytes.len() >= 6 && bytes[..4] == [0, 0, 1, 0] => {
            let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
            if count == 0 || count > 256 || 6 + count * 16 > bytes.len() {
                return Err(bad());
            }
            // ICO directory dimensions are at most 256 pixels. Embedded PNGs also
            // need their real dimensions checked rather than trusting that directory.
            for index in 0..count {
                let at = 6 + index * 16;
                let size = u32::from_le_bytes(bytes[at + 8..at + 12].try_into().unwrap()) as usize;
                let start =
                    u32::from_le_bytes(bytes[at + 12..at + 16].try_into().unwrap()) as usize;
                let data = bytes
                    .get(start..start.checked_add(size).ok_or_else(bad)?)
                    .ok_or_else(bad)?;
                if data.starts_with(b"\x89PNG\r\n\x1a\n") {
                    validate_image(data, "png")?;
                } else if data.len() >= 12
                    && u32::from_le_bytes(data[..4].try_into().unwrap()) >= 40
                {
                    bounded(
                        i32::from_le_bytes(data[4..8].try_into().unwrap()).unsigned_abs() as u64,
                        i32::from_le_bytes(data[8..12].try_into().unwrap()).unsigned_abs() as u64,
                    )?;
                } else {
                    return Err(bad());
                }
            }
            Ok(())
        }
        "avif" => {
            // Every ISO BMFF spatial extent, including grids/auxiliary images, must
            // be small; do not trust just the first extent of a composite image.
            let mut found = false;
            for (index, marker) in bytes.windows(4).enumerate() {
                if marker != b"ispe" {
                    continue;
                }
                let data = bytes.get(index + 8..index + 16).ok_or_else(bad)?;
                bounded(
                    u32::from_be_bytes(data[..4].try_into().unwrap()) as u64,
                    u32::from_be_bytes(data[4..].try_into().unwrap()) as u64,
                )?;
                found = true;
            }
            if found {
                Ok(())
            } else {
                Err(bad())
            }
        }
        "png" | "jpg" | "jpeg" | "webp" => {
            let reader = image::ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()
                .map_err(|_| bad())?;
            let (width, height) = reader.into_dimensions().map_err(|_| bad())?;
            bounded(width as u64, height as u64)
        }
        _ => Err(bad()),
    }
}

fn validate_archive(bytes: &[u8]) -> Result<(), String> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|_| "The document archive is damaged or unsupported")?;
    if archive.is_empty() || archive.len() > ARCHIVE_ENTRIES {
        return Err("The document has too many archive entries".into());
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 32 * 1024];
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| "The document archive cannot be read")?;
        let name = entry
            .name()
            .map_err(|_| "The document has an invalid archive path")?
            .into_owned();
        if entry.enclosed_name().is_none()
            || name.starts_with('/')
            || name.split('/').any(|part| matches!(part, "." | ".."))
            || name.contains('\\')
            || !names.insert(name)
        {
            return Err("The document contains ambiguous archive paths".into());
        }
        if entry.is_symlink() || entry.size() > ENTRY_BYTES || entry.compressed_size() > FILE_BYTES
        {
            return Err("The document archive exceeds the preview limits".into());
        }
        let declared = entry.size();
        let mut expanded = 0u64;
        let mut raster: Option<(&str, Vec<u8>)> = None;
        loop {
            let count = entry
                .read(&mut buffer)
                .map_err(|_| "The document archive is damaged")?;
            if count == 0 {
                break;
            }
            expanded += count as u64;
            total += count as u64;
            if expanded > ENTRY_BYTES || total > ARCHIVE_BYTES {
                return Err("The document unpacks beyond the 64 MiB preview limit".into());
            }
            if expanded == count as u64 {
                let head = &buffer[..count];
                let kind = if head.starts_with(b"\x89PNG\r\n\x1a\n") {
                    Some("png")
                } else if head.starts_with(b"\xff\xd8\xff") {
                    Some("jpeg")
                } else if head.starts_with(b"GIF8") {
                    Some("gif")
                } else if head.starts_with(b"BM") {
                    Some("bmp")
                } else if head.starts_with(&[0, 0, 1, 0]) {
                    Some("ico")
                } else if head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
                    Some("webp")
                } else if head.get(4..8) == Some(b"ftyp")
                    && head.windows(4).any(|brand| brand == b"avif")
                {
                    Some("avif")
                } else {
                    None
                };
                raster = kind.map(|kind| (kind, Vec::new()));
            }
            if let Some((_, data)) = raster.as_mut() {
                if expanded > 8 * 1024 * 1024 {
                    return Err("An embedded image exceeds the 8 MiB preview limit".into());
                }
                data.extend_from_slice(&buffer[..count]);
            }
        }
        if expanded != declared {
            return Err("The document archive has inconsistent entry sizes".into());
        }
        if let Some((kind, data)) = raster {
            validate_image(&data, kind)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn validates_real_archive_content_and_rejects_ambiguous_paths() {
        assert!(validate_archive(&archive(&[
            ("word/document.xml", b"<document/>"),
            ("media/picture.png", b"image")
        ]))
        .is_ok());
        for name in ["../escape.xml", "/absolute.xml", "word\\document.xml"] {
            // ZipWriter normalizes filenames; inject the hostile raw name into
            // both headers so this exercises the reader, not the writer's policy.
            let placeholder = "x".repeat(name.len());
            let mut bytes = archive(&[(&placeholder, b"x")]);
            let offsets: Vec<_> = bytes
                .windows(placeholder.len())
                .enumerate()
                .filter(|(_, window)| *window == placeholder.as_bytes())
                .map(|(index, _)| index)
                .collect();
            assert_eq!(offsets.len(), 2);
            for offset in offsets {
                bytes[offset..offset + name.len()].copy_from_slice(name.as_bytes());
            }
            assert!(validate_archive(&bytes).is_err(), "{name}");
        }
        assert!(validate_archive(b"not a ZIP").is_err());
    }

    #[test]
    fn rejects_oversized_and_falsified_archive_lengths() {
        let oversized = archive(&[("word/document.xml", &vec![b'a'; ENTRY_BYTES as usize + 1])]);
        assert!(validate_archive(&oversized).is_err());
        let mut forged = archive(&[("word/document.xml", b"actual content")]);
        let central = forged
            .windows(4)
            .position(|window| window == b"PK\x01\x02")
            .unwrap();
        forged[central + 24..central + 28].copy_from_slice(&1u32.to_le_bytes());
        assert!(validate_archive(&forged).is_err());
    }

    #[test]
    fn rejects_large_image_dimensions_before_the_webview_decodes_them() {
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&10_000u16.to_le_bytes());
        gif.extend_from_slice(&10_000u16.to_le_bytes());
        assert!(validate_image(&gif, "gif").is_err());
        assert!(validate_archive(&archive(&[("word/media/disguised.xml", &gif)])).is_err());
        gif[6..8].copy_from_slice(&400u16.to_le_bytes());
        gif[8..10].copy_from_slice(&300u16.to_le_bytes());
        assert!(validate_image(&gif, "gif").is_ok());
        assert!(validate_image(b"not an image", "png").is_err());
        assert!(validate_image(&vec![0; 8 * 1024 * 1024 + 1], "svg").is_err());
    }

    #[test]
    fn reads_only_regular_documents_inside_the_workspace() {
        let root =
            std::env::temp_dir().join(format!("pandacode-document-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join(".git")).unwrap();
        let root = fs::canonicalize(root).unwrap();
        fs::write(root.join("Report.PDF"), b"%PDF-1.7\n").unwrap();
        fs::write(root.join("notes.txt"), b"notes").unwrap();
        fs::write(root.join(".git/private.pdf"), b"private").unwrap();
        assert_eq!(
            read_document_bytes(&root, "Report.PDF").unwrap(),
            b"%PDF-1.7\n"
        );
        for path in [
            "notes.txt",
            "../outside.pdf",
            ".git/private.pdf",
            "missing.pdf",
        ] {
            assert!(read_document_bytes(&root, path).is_err(), "{path}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join(".git/private.pdf"), root.join("alias.pdf"))
                .unwrap();
            assert!(read_document_bytes(&root, "alias.pdf").is_err());
            std::os::unix::fs::symlink(root.join("notes.txt"), root.join("notes.pdf")).unwrap();
            assert!(read_document_bytes(&root, "notes.pdf").is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn opened_handle_must_match_the_authorized_non_protected_path() {
        let root = std::env::temp_dir().join("document-handle-boundary");
        let relative = Path::new("docs/report.pdf");
        assert!(validate_opened_path(&root, relative, &root.join(relative)).is_ok());
        assert!(validate_opened_path(&root, relative, &root.join(".git/private.pdf")).is_err());
        assert!(
            validate_opened_path(&root, relative, &root.with_file_name("outside.pdf")).is_err()
        );
        assert!(validate_opened_path(&root, relative, &root.join("other.pdf")).is_err());
        for path in [".git/private.pdf", ".GIT/private.pdf", "docs/notes.txt"] {
            assert!(validate_opened_path(&root, Path::new(path), &root.join(path)).is_err());
        }
    }
}
