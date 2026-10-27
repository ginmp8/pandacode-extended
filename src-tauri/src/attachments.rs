//! Durable, task-scoped links to prompt images for Connect.
use crate::storage::Store;
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{
    codecs::jpeg::JpegEncoder, imageops::FilterType, DynamicImage, ImageFormat, ImageReader, Limits,
};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Cursor, Read, Write},
    path::{Component, Path, PathBuf},
};

pub const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_RAW_CHUNK: usize = 120 * 1024;
const MAX_PROMPT_IMAGE: u64 = 5 * 1024 * 1024;
const MAX_MOODBOARD_IMAGE: u64 = 12 * 1024 * 1024;
const MESSAGE_PAGE_BUDGET: usize = 180 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub mime: String,
    pub bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Thumb,
    Full,
}

impl Variant {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "thumb" => Ok(Self::Thumb),
            "full" => Ok(Self::Full),
            _ => Err("Invalid variant.".into()),
        }
    }
}

/// Called after the user message row exists. Each failed attachment is logged and skipped so it
/// cannot turn a successfully saved prompt into a failed send.
pub fn link_message_attachments(
    store: &Store,
    session_id: &str,
    message_id: i64,
    content: &str,
    image_ids: &[String],
    capture_dir: &Path,
    app_data_dir: &Path,
) -> Result<usize, String> {
    let owns_message: bool = store.db.query_row(
        "SELECT EXISTS(SELECT 1 FROM messages WHERE id=?1 AND session_id=?2)",
        params![message_id, session_id], |row| row.get(0),
    ).map_err(|error| error.to_string())?;
    if !owns_message {
        return Err("The attachment message does not belong to this task.".into());
    }
    let project_id: String = store
        .db
        .query_row(
            "SELECT project_id FROM sessions WHERE id=?1",
            [session_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or("Conversation not found.")?;
    let names = prompt_image_names(content);
    let mut linked = 0;
    for (index, id) in image_ids.iter().enumerate() {
        match link_capture(
            store,
            session_id,
            message_id,
            id,
            names.get(index).map(String::as_str),
            capture_dir,
            app_data_dir,
        ) {
            Ok(()) => linked += 1,
            Err(error) => eprintln!("PandaCode: could not persist prompt image {id}: {error}"),
        }
    }
    for board_id in moodboard_ids(content) {
        let rows = store.db.prepare(
            "SELECT i.id,i.name,i.mime_type,i.width,i.height,length(i.original)
             FROM moodboard_images i JOIN moodboards b ON b.id=i.board_id
             WHERE b.id=?1 AND b.project_id=?2 ORDER BY i.created_at,i.id",
        );
        let rows = match rows {
            Ok(mut query) => match query.query_map(params![board_id, project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, u32>(3)?,
                    row.get::<_, u32>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            }) {
                Ok(rows) => rows.collect::<rusqlite::Result<Vec<_>>>(),
                Err(error) => Err(error),
            },
            Err(error) => Err(error),
        };
        let rows = match rows {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!(
                    "PandaCode: could not list moodboard attachments for {board_id}: {error}"
                );
                continue;
            }
        };
        for (id, name, mime, width, height, bytes) in rows {
            if bytes < 0 || bytes as u64 > MAX_MOODBOARD_IMAGE {
                eprintln!("PandaCode: moodboard image {id} exceeds the stored attachment limit");
                continue;
            }
            let result = store.db.execute(
                "INSERT OR IGNORE INTO message_attachments
                 (message_id,session_id,id,kind,name,mime,width,height,bytes,file,source,moodboard_id,moodboard_image_id)
                 VALUES(?1,?2,?3,'image',?4,?5,?6,?7,?8,NULL,'moodboard',?9,?3)",
                params![message_id, session_id, id, name, mime, width, height, bytes, board_id],
            );
            match result {
                Ok(count) => linked += count,
                Err(error) => eprintln!("PandaCode: could not link moodboard image {id}: {error}"),
            }
        }
    }
    Ok(linked)
}

fn prompt_image_names(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let rest = line
                .strip_prefix("Attached browser screenshot: ")
                .or_else(|| line.strip_prefix("Attached image: "))
                .or_else(|| line.strip_prefix("Attached mockup: "))?;
            let name = rest.split(" (").next().unwrap_or(rest).trim();
            (!name.is_empty()).then(|| name.chars().take(200).collect())
        })
        .collect()
}

fn moodboard_ids(content: &str) -> Vec<String> {
    let mut result = Vec::new();
    for part in content.split("(moodboard ID: ").skip(1) {
        let Some(id) = part.split(')').next() else {
            continue;
        };
        if canonical_uuid(id) && !result.iter().any(|seen| seen == id) {
            result.push(id.to_owned());
        }
    }
    result
}

fn canonical_uuid(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id)
}

fn link_capture(
    store: &Store,
    session_id: &str,
    message_id: i64,
    id: &str,
    name: Option<&str>,
    capture_dir: &Path,
    app_data_dir: &Path,
) -> Result<(), String> {
    if !canonical_uuid(id) || !canonical_uuid(session_id) {
        return Err("invalid attachment id".into());
    }
    let source = capture_dir.join(format!("{id}.png"));
    let bytes = read_bounded_file(&source, MAX_PROMPT_IMAGE)?;
    let (width, height) = image_dimensions(&bytes)?;
    let name = name
        .filter(|name| !name.is_empty())
        .unwrap_or("Image attachment");
    let attachment_root = app_data_dir.join("attachments");
    let session_dir = attachment_root.join(session_id);
    // The application data directory is chosen by Tauri, never by a peer. Create
    // children one at a time so an existing link is rejected before following it.
    ensure_private_directory(app_data_dir)?;
    ensure_private_directory(&attachment_root)?;
    ensure_private_directory(&session_dir)?;
    let destination = session_dir.join(format!("{id}.png"));
    if let Ok(metadata) = fs::symlink_metadata(&destination) {
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() != bytes.len() as u64
        {
            return Err("destination attachment already exists".into());
        }
        if read_bounded_file(&destination, MAX_PROMPT_IMAGE)? != bytes {
            return Err("destination attachment id collision".into());
        }
    } else {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&destination).map_err(|error| error.to_string())?;
        file.write_all(&bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
    }
    let file = format!("{session_id}/{id}.png");
    store
        .db
        .execute(
            "INSERT OR IGNORE INTO message_attachments
         (message_id,session_id,id,kind,name,mime,width,height,bytes,file,source,moodboard_id,moodboard_image_id)
         VALUES(?1,?2,?3,'image',?4,'image/png',?5,?6,?7,?8,'file',NULL,NULL)",
            params![
                message_id,
                session_id,
                id,
                name,
                width,
                height,
                bytes.len() as i64,
                file
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn image_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    let format = image::guess_format(bytes).map_err(|error| error.to_string())?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err("unsupported image format".into());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    reader.limits(limits);
    reader.into_dimensions().map_err(|error| error.to_string())
}

fn ensure_private_directory(path: &Path) -> Result<(), String> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    if let Err(error) = builder.create(path) {
        if error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(error.to_string());
        }
    }
    regular_directory(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn regular_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("attachment directory is not a regular directory".into());
    }
    Ok(())
}

/// Read from the opened regular file, with an explicit cap even if it grows.
pub(crate) fn read_bounded_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    if !fs::symlink_metadata(path).map_err(|error| error.to_string())?.file_type().is_file() {
        return Err("attachment is not a regular file".into());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > limit {
        return Err("attachment is not a bounded regular file".into());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(limit + 1).read_to_end(&mut bytes).map_err(|error| error.to_string())?;
    if bytes.is_empty() || bytes.len() as u64 > limit {
        return Err("attachment exceeds its size limit".into());
    }
    Ok(bytes)
}

/// Remove only the private directory of an already deleted, UUID-named task.
pub(crate) fn remove_task_files(app_data_dir: &Path, session_id: &str) -> Result<(), String> {
    if !canonical_uuid(session_id) {
        return Err("invalid attachment task id".into());
    }
    let root = app_data_dir.join("attachments");
    if !root.try_exists().map_err(|error| error.to_string())? {
        return Ok(());
    }
    regular_directory(&root)?;
    let directory = root.join(session_id);
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
        Ok(_) => regular_directory(&directory)?,
    }
    fs::remove_dir_all(directory).map_err(|error| error.to_string())
}

#[derive(Debug)]
pub struct ImageBytes {
    pub metadata: Metadata,
    pub bytes: Vec<u8>,
}

/// Resolves a UUID only through a session-owned row; callers must already have refused mirrors.
pub fn load_attachment(
    db: &rusqlite::Connection,
    app_data_dir: &Path,
    session_id: &str,
    id: &str,
) -> Result<ImageBytes, String> {
    if !canonical_uuid(id) || !canonical_uuid(session_id) {
        return Err("Attachment is unavailable.".into());
    }
    let row = db
        .query_row(
            "SELECT id,kind,name,mime,bytes,width,height,file,source,moodboard_id,moodboard_image_id
         FROM message_attachments a WHERE a.session_id=?1 AND a.id=?2
           AND EXISTS(SELECT 1 FROM messages m WHERE m.id=a.message_id AND m.session_id=a.session_id)",
            params![session_id, id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, Option<u32>>(5)?,
                    row.get::<_, Option<u32>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?
        .ok_or("Attachment is unavailable.")?;
    let (
        id,
        kind,
        name,
        mime,
        stored_bytes,
        width,
        height,
        file,
        source,
        moodboard_id,
        moodboard_image_id,
    ) = row;
    let bytes = u64::try_from(stored_bytes).map_err(|_| "Attachment is unavailable.")?;
    if kind != "image" || !mime.starts_with("image/") {
        return Err("Attachment is unavailable.".into());
    }
    let data = match source.as_str() {
        "file" => {
            let relative = file.ok_or("Attachment is unavailable.")?;
            let expected = PathBuf::from(session_id).join(format!("{id}.png"));
            let relative_path = PathBuf::from(&relative);
            if relative_path != expected
                || !relative_path
                    .components()
                    .all(|part| matches!(part, Component::Normal(_)))
            {
                return Err("Attachment is unavailable.".into());
            }
            let attachment_root = app_data_dir.join("attachments");
            let session_dir = attachment_root.join(session_id);
            for directory in [&attachment_root, &session_dir] {
                let metadata =
                    fs::symlink_metadata(directory).map_err(|_| "Attachment is unavailable.")?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err("Attachment is unavailable.".into());
                }
            }
            let path = session_dir.join(
                relative_path
                    .file_name()
                    .ok_or("Attachment is unavailable.")?,
            );
            let metadata = fs::symlink_metadata(&path).map_err(|_| "Attachment is unavailable.")?;
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.len() > MAX_PROMPT_IMAGE
            {
                return Err("Attachment is unavailable.".into());
            }
            let data = read_bounded_file(&path, MAX_PROMPT_IMAGE).map_err(|_| "Attachment is unavailable.")?;
            if data.len() as u64 != bytes {
                return Err("Attachment is unavailable.".into());
            }
            data
        }
        "moodboard" => {
            let moodboard_id = moodboard_id.ok_or("Attachment is unavailable.")?;
            let moodboard_image_id = moodboard_image_id.ok_or("Attachment is unavailable.")?;
            let data: Vec<u8> = db.query_row(
                "SELECT i.original FROM moodboard_images i JOIN moodboards b ON b.id=i.board_id
                 WHERE i.id=?1 AND b.id=?2 AND EXISTS(
                   SELECT 1 FROM message_attachments a WHERE a.session_id=?3 AND a.id=?4 AND a.source='moodboard'
                     AND a.moodboard_id=b.id AND a.moodboard_image_id=i.id
                     AND EXISTS(SELECT 1 FROM messages m WHERE m.id=a.message_id AND m.session_id=a.session_id)
                 )",
                params![moodboard_image_id, moodboard_id, session_id, id], |row| row.get(0),
            ).optional().map_err(|error| error.to_string())?.ok_or("Attachment is unavailable.")?;
            if data.len() as u64 != bytes || data.len() as u64 > MAX_MOODBOARD_IMAGE {
                return Err("Attachment is unavailable.".into());
            }
            data
        }
        _ => return Err("Attachment is unavailable.".into()),
    };
    Ok(ImageBytes {
        metadata: Metadata {
            id,
            kind,
            name,
            mime,
            bytes,
            width,
            height,
        },
        bytes: data,
    })
}

pub fn variant_bytes(
    bytes: &[u8],
    mime: &str,
    variant: Variant,
) -> Result<(String, Vec<u8>), String> {
    if variant == Variant::Full && bytes.len() <= MAX_SOURCE_BYTES {
        return Ok((mime.to_owned(), bytes.to_vec()));
    }
    let image = decode(bytes)?;
    match variant {
        Variant::Thumb => Ok(("image/jpeg".into(), encode_jpeg_bounded(&image)?)),
        Variant::Full => Ok(("image/jpeg".into(), encode_jpeg(&image, 2048, 84)?)),
    }
}

fn decode(bytes: &[u8]) -> Result<DynamicImage, String> {
    let format = image::guess_format(bytes)
        .map_err(|_| "This image cannot be decoded for a preview.".to_string())?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err("This image format cannot be converted to a JPEG preview.".into());
    }
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|_| "This image cannot be decoded for a preview.".into())
}

fn encode_jpeg(image: &DynamicImage, edge: u32, quality: u8) -> Result<Vec<u8>, String> {
    let scaled = if image.width() > edge || image.height() > edge {
        image.resize(edge, edge, FilterType::Lanczos3)
    } else {
        image.clone()
    };
    let mut bytes = Vec::new();
    JpegEncoder::new_with_quality(&mut bytes, quality)
        .encode_image(&scaled.to_rgb8())
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

fn encode_jpeg_bounded(image: &DynamicImage) -> Result<Vec<u8>, String> {
    for edge in [480, 400, 320, 240] {
        for quality in [82, 70, 58, 46, 36] {
            let bytes = encode_jpeg(image, edge, quality)?;
            if bytes.len() <= MAX_RAW_CHUNK {
                return Ok(bytes);
            }
        }
    }
    Err("Could not fit the image preview into one Connect chunk.".into())
}

pub fn chunk_reply(bytes: &[u8], mime: &str, offset: u64) -> Result<Value, String> {
    let start = usize::try_from(offset).map_err(|_| "Invalid offset.")?;
    if start > bytes.len() {
        return Err("Invalid offset.".into());
    }
    let end = start.saturating_add(MAX_RAW_CHUNK).min(bytes.len());
    let data = STANDARD.encode(&bytes[start..end]);
    let response = json!({ "mime": mime, "total": bytes.len(), "offset": start, "data": data, "done": end == bytes.len() });
    if response.to_string().len() > MESSAGE_PAGE_BUDGET {
        return Err("Attachment chunk exceeds the Connect reply budget.".into());
    }
    Ok(response)
}

/// Metadata rows for one Connect message, in their original attachment order.
pub fn message_attachment_rows(
    db: &rusqlite::Connection,
    session_id: &str,
    message_id: i64,
) -> Result<Vec<Value>, String> {
    let mut statement = db
        .prepare(
            "SELECT id,kind,name,mime,bytes,width,height FROM message_attachments
         WHERE session_id=?1 AND message_id=?2 ORDER BY rowid",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![session_id, message_id], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?, "kind": row.get::<_, String>(1)?, "name": row.get::<_, String>(2)?,
            "mime": row.get::<_, String>(3)?, "bytes": row.get::<_, i64>(4)?,
            "width": row.get::<_, Option<u32>>(5)?, "height": row.get::<_, Option<u32>>(6)?
        }))
    }).map_err(|error| error.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
pub fn message_attachment_rows(
    db: &rusqlite::Connection,
    session_id: &str,
    message_id: i64,
) -> Result<Vec<Value>, String> {
    let mut statement = db
        .prepare(
            "SELECT id,kind,name,mime,bytes,width,height FROM message_attachments
         WHERE session_id=?1 AND message_id=?2 ORDER BY rowid",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement.query_map(params![session_id, message_id], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?, "kind": row.get::<_, String>(1)?, "name": row.get::<_, String>(2)?,
            "mime": row.get::<_, String>(3)?, "bytes": row.get::<_, i64>(4)?,
            "width": row.get::<_, Option<u32>>(5)?, "height": row.get::<_, Option<u32>>(6)?
        }))
    }).map_err(|error| error.to_string())?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pandacode-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, Rgba([255, 10, 20, 255]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .unwrap();
        bytes
    }

    fn task(store: &Store, root: &Path) -> String {
        let project_dir = root.join(format!("project-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&project_dir).unwrap();
        let project = store.add_project(&project_dir).unwrap();
        store
            .create_session(&project.id, "codex", "default", "ask")
            .unwrap()
            .id
    }

    #[test]
    fn linking_copies_capture_and_persists_metadata_in_a_temp_directory() {
        let root = temp_dir("attachment-link");
        let store = Store::open(&root.join("db.sqlite3")).unwrap();
        let session = task(&store, &root);
        let message = store
            .start_turn_message(&session, "Attached image: cabin.png (8 × 4)")
            .unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let captures = root.join("cache/browser-captures");
        fs::create_dir_all(&captures).unwrap();
        fs::write(captures.join(format!("{id}.png")), png(8, 4)).unwrap();
        let linked = link_message_attachments(
            &store,
            &session,
            message,
            "Attached image: cabin.png (8 × 4)",
            std::slice::from_ref(&id),
            &captures,
            &root.join("data"),
        )
        .unwrap();
        assert_eq!(linked, 1);
        let item = message_attachment_rows(&store.db, &session, message).unwrap();
        assert_eq!(item[0]["id"], id);
        assert_eq!(item[0]["name"], "cabin.png");
        assert_eq!(item[0]["width"].as_u64(), Some(8));
        assert_eq!(item[0]["height"].as_u64(), Some(4));
        assert!(root
            .join(format!("data/attachments/{session}/{id}.png"))
            .is_file());
        let loaded = load_attachment(&store.db, &root.join("data"), &session, &id).unwrap();
        assert_eq!(loaded.bytes, png(8, 4));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_foreign_ids_and_foreign_sessions() {
        let root = temp_dir("attachment-owner");
        let store = Store::open(&root.join("db.sqlite3")).unwrap();
        let session = task(&store, &root);
        let foreign = task(&store, &root);
        let message = store
            .start_turn_message(&session, "Attached image: shot.png")
            .unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let captures = root.join("cache");
        fs::create_dir_all(&captures).unwrap();
        fs::write(captures.join(format!("{id}.png")), png(2, 2)).unwrap();
        link_message_attachments(
            &store,
            &session,
            message,
            "Attached image: shot.png",
            std::slice::from_ref(&id),
            &captures,
            &root.join("data"),
        )
        .unwrap();
        assert!(load_attachment(
            &store.db,
            &root.join("data"),
            &session,
            &uuid::Uuid::new_v4().to_string()
        )
        .is_err());
        assert!(load_attachment(&store.db, &root.join("data"), &foreign, &id).is_err());
        assert!(
            load_attachment(&store.db, &root.join("data"), &session, "../../etc/passwd").is_err()
        );
        let foreign_message = store.start_turn_message(&foreign, "Other task").unwrap();
        assert!(link_message_attachments(
            &store, &session, foreign_message, "", std::slice::from_ref(&id), &captures, &root.join("data"),
        ).is_err(), "message ownership is checked before copying");
        store.db.execute(
            "UPDATE message_attachments SET message_id=?1 WHERE session_id=?2 AND id=?3",
            params![foreign_message, session, id],
        ).unwrap();
        assert!(load_attachment(&store.db, &root.join("data"), &session, &id).is_err(), "a mismatched stored message cannot authorize a read");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn attachment_storage_refuses_symlink_directories_and_files() {
        use std::os::unix::fs::symlink;
        let root = temp_dir("attachment-links");
        let outside = root.join("outside");
        let data = root.join("data");
        fs::create_dir(&outside).unwrap();
        fs::create_dir(&data).unwrap();
        symlink(&outside, data.join("attachments")).unwrap();
        assert!(ensure_private_directory(&data.join("attachments")).is_err());
        let id = uuid::Uuid::new_v4().to_string();
        assert!(remove_task_files(&data, &id).is_err());
        assert!(remove_task_files(&data, "../outside").is_err());
        let bytes = png(2, 2);
        fs::write(outside.join("image.png"), &bytes).unwrap();
        symlink(outside.join("image.png"), data.join("image.png")).unwrap();
        assert!(read_bounded_file(&data.join("image.png"), MAX_PROMPT_IMAGE).is_err());
        assert_eq!(read_bounded_file(&outside.join("image.png"), MAX_PROMPT_IMAGE).unwrap(), bytes);
        assert!(read_bounded_file(&outside.join("image.png"), 1).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn thumbnail_is_jpeg_with_a_long_edge_no_larger_than_480() {
        let (mime, bytes) = variant_bytes(&png(128, 64), "image/png", Variant::Thumb).unwrap();
        assert_eq!(mime, "image/jpeg");
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (128, 64));
        assert!(decoded.width().max(decoded.height()) <= 480);
        assert!(bytes.len() <= MAX_RAW_CHUNK);
    }

    #[test]
    fn chunks_are_limited_and_fit_the_encrypted_reply_budget() {
        let bytes = vec![7u8; MAX_RAW_CHUNK + 100];
        let first = chunk_reply(&bytes, "image/png", 0).unwrap();
        let second = chunk_reply(&bytes, "image/png", MAX_RAW_CHUNK as u64).unwrap();
        assert_eq!(
            STANDARD
                .decode(first["data"].as_str().unwrap())
                .unwrap()
                .len(),
            MAX_RAW_CHUNK
        );
        assert_eq!(
            STANDARD
                .decode(second["data"].as_str().unwrap())
                .unwrap()
                .len(),
            100
        );
        assert!(first.to_string().len() <= MESSAGE_PAGE_BUDGET);
        assert_eq!(first["done"], false);
        assert_eq!(second["done"], true);
        assert!(chunk_reply(&bytes, "image/png", bytes.len() as u64 + 1).is_err());
    }

    #[test]
    fn moodboard_blobs_are_linked_without_copying_the_original() {
        let root = temp_dir("attachment-moodboard");
        let store = Store::open(&root.join("db.sqlite3")).unwrap();
        let session = task(&store, &root);
        let message = store.start_turn_message(&session, "The user attached these project moodboards:\n- Board (moodboard ID: board-id, 1 images)").unwrap();
        let project: String = store
            .db
            .query_row(
                "SELECT project_id FROM sessions WHERE id=?1",
                [&session],
                |row| row.get(0),
            )
            .unwrap();
        let board = uuid::Uuid::new_v4().to_string();
        let image_id = uuid::Uuid::new_v4().to_string();
        store.db.execute("INSERT INTO moodboards(id,project_id,name,created_at,updated_at) VALUES(?1,?2,'Board',1,1)", params![board,project]).unwrap();
        store.db.execute("INSERT INTO moodboard_images(id,board_id,name,caption,mime_type,width,height,original,thumbnail,agent_image,created_at) VALUES(?1,?2,'board.png','', 'image/png',2,2,?3,?3,?3,1)", params![image_id,board,png(2,2)]).unwrap();
        let content = format!("(moodboard ID: {board})");
        assert_eq!(
            link_message_attachments(
                &store,
                &session,
                message,
                &content,
                &[],
                &root,
                &root.join("data")
            )
            .unwrap(),
            1
        );
        let stored_board: String = store
            .db
            .query_row(
                "SELECT moodboard_id FROM message_attachments WHERE session_id=?1 AND id=?2",
                params![session, image_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stored_board, board);
        let loaded = load_attachment(&store.db, &root.join("data"), &session, &image_id).unwrap();
        assert_eq!(loaded.bytes, png(2, 2));
        assert!(!root
            .join(format!("data/attachments/{session}/{image_id}.png"))
            .exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn image_id_and_variant_validation_are_strict() {
        assert!(!canonical_uuid("../file.png"));
        assert_eq!(Variant::parse("thumb"), Ok(Variant::Thumb));
        assert_eq!(Variant::parse("full"), Ok(Variant::Full));
        assert!(Variant::parse("path").is_err());
    }
}
