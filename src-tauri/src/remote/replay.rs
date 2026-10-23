//! Durable admission receipts for encrypted Connect messages. Only a nonce hash
//! and timestamps are stored; a receipt commits before any request/event runs.
use crate::Result;
use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const WINDOW_MS: i64 = 60_000;
const MAX_SEEN: i64 = 20_000;
const REFUSED: &str = "This message expired or was already received. Check that both computers' clocks are right.";
const UNAVAILABLE: &str = "Connect replay protection is unavailable. No remote message was admitted.";

/// Default is deliberately unavailable, never an ephemeral fallback. Production
/// must open its durable database before it may admit an authenticated message.
#[derive(Default)]
pub(crate) struct Replay {
    db: Option<Connection>,
}

impl Replay {
    pub(crate) fn open(app_dir: &Path) -> Result<Self> {
        let path = private_file(app_dir).map_err(|error| format!("Could not prepare Connect replay storage: {error}"))?;
        // The file was created privately above. Do not follow a substituted link
        // or silently create another database if it disappeared meanwhile.
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_NOFOLLOW;
        let db = Connection::open_with_flags(path, flags).map_err(|error| format!("Could not open Connect replay storage: {error}"))?;
        Self::initialize(db)
    }

    fn initialize(db: Connection) -> Result<Self> {
        db.busy_timeout(std::time::Duration::from_secs(1)).map_err(|_| UNAVAILABLE.to_owned())?;
        db.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA;
             PRAGMA cache_size=-1024; PRAGMA mmap_size=0; PRAGMA temp_store=MEMORY;
             PRAGMA max_page_count=2048; PRAGMA trusted_schema=OFF;
             CREATE TABLE IF NOT EXISTS receipts (
                 nonce_hash BLOB PRIMARY KEY NOT NULL CHECK(length(nonce_hash)=32),
                 ts INTEGER NOT NULL
             ) WITHOUT ROWID;
             CREATE INDEX IF NOT EXISTS receipts_ts ON receipts(ts);
             CREATE TABLE IF NOT EXISTS replay_clock (
                 id INTEGER PRIMARY KEY CHECK(id=1), last_ms INTEGER NOT NULL
             );
             INSERT OR IGNORE INTO replay_clock(id,last_ms) VALUES(1,-9223372036854775808);",
        ).map_err(|_| UNAVAILABLE.to_owned())?;
        Ok(Self { db: Some(db) })
    }

    pub(crate) fn check(&mut self, ts: Option<i64>, nonce: Option<&str>, now_ms: i64) -> Result<()> {
        let (Some(ts), Some(nonce)) = (ts, nonce) else { return Err(REFUSED.into()) };
        if !(16..=64).contains(&nonce.len()) || !nonce.bytes().all(|byte| byte.is_ascii_graphic()) || now_ms.abs_diff(ts) > WINDOW_MS as u64 {
            return Err(REFUSED.into());
        }
        let db = self.db.as_mut().ok_or(UNAVAILABLE)?;
        // IMMEDIATE also serializes two processes accidentally using one data
        // directory. Memory-only duplicate checks could admit the same nonce twice.
        let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(|_| UNAVAILABLE.to_owned())?;
        let last_ms: i64 = transaction.query_row("SELECT last_ms FROM replay_clock WHERE id=1", [], |row| row.get(0)).map_err(|_| UNAVAILABLE.to_owned())?;
        if now_ms < last_ms {
            // A backward clock must not make already-pruned ciphertext fresh
            // again. This watermark is committed atomically with pruning.
            return Err("The clock moved backwards. Connect will refuse messages until it catches up.".into());
        }
        transaction.execute("DELETE FROM receipts WHERE ts<?1", [now_ms.saturating_sub(WINDOW_MS)]).map_err(|_| UNAVAILABLE.to_owned())?;
        let hash = Sha256::digest(nonce.as_bytes());
        let seen: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM receipts WHERE nonce_hash=?1)", [hash.as_slice()], |row| row.get(0)).map_err(|_| UNAVAILABLE.to_owned())?;
        if seen {
            return Err(REFUSED.into());
        }
        let count: i64 = transaction.query_row("SELECT COUNT(*) FROM receipts", [], |row| row.get(0)).map_err(|_| UNAVAILABLE.to_owned())?;
        if count >= MAX_SEEN {
            // Never evict a still-valid receipt to make room for more work.
            return Err("Connect replay protection is at capacity. Try again in a moment.".into());
        }
        transaction.execute("INSERT INTO receipts(nonce_hash,ts) VALUES(?1,?2)", params![hash.as_slice(), ts]).map_err(|_| UNAVAILABLE.to_owned())?;
        transaction.execute("UPDATE replay_clock SET last_ms=?1 WHERE id=1", [now_ms]).map_err(|_| UNAVAILABLE.to_owned())?;
        transaction.commit().map_err(|_| UNAVAILABLE.to_owned())
    }

    #[cfg(test)]
    pub(crate) fn memory() -> Self {
        Self::initialize(Connection::open_in_memory().unwrap()).unwrap()
    }
}

/// Keep SQLite and its rollback journal together in an owner-only directory on
/// Unix. Windows uses the application data directory's inherited user ACL.
fn private_file(app_dir: &Path) -> std::io::Result<PathBuf> {
    let dir = app_dir.join("connect-replay");
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    let new_dir = match builder.create(&dir) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error),
    };
    if !std::fs::symlink_metadata(&dir)?.file_type().is_dir() {
        return Err(std::io::Error::other("Replay storage must be a real directory."));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let path = dir.join("receipts.sqlite");
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_file() && metadata.len() > 0 => {}
        Ok(_) => return Err(std::io::Error::other("Replay storage is not an intact regular file.")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && new_dir => { options.open(&path)?; }
        Err(error) => return Err(error),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Folder(PathBuf);

    impl Folder {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("pandacode-replay-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn admitted_nonces_survive_restart_and_a_second_connection() {
        let folder = Folder::new();
        let now = 1_700_000_000_000;
        let nonce = "0123456789abcdef0123";
        {
            let mut first = Replay::open(&folder.0).unwrap();
            assert!(first.check(Some(now), Some(nonce), now).is_ok());
            let (hash, ts): (Vec<u8>, i64) = first.db.as_ref().unwrap().query_row("SELECT nonce_hash,ts FROM receipts", [], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
            assert_eq!(hash, Sha256::digest(nonce.as_bytes()).as_slice());
            assert_eq!(ts, now);
            let mut second = Replay::open(&folder.0).unwrap();
            assert!(second.check(Some(now), Some(nonce), now + 1).is_err());
        }
        let mut restarted = Replay::open(&folder.0).unwrap();
        assert!(restarted.check(Some(now), Some(nonce), now + 2).is_err());
        assert!(restarted.check(Some(now), Some("different-nonce-0001"), now + 2).is_ok());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(folder.0.join("connect-replay")).unwrap().permissions().mode() & 0o777, 0o700);
            assert_eq!(std::fs::metadata(folder.0.join("connect-replay/receipts.sqlite")).unwrap().permissions().mode() & 0o777, 0o600);
        }
    }

    #[test]
    fn unavailable_or_unwritable_receipts_never_admit_a_message() {
        let now = 1_700_000_000_000;
        let nonce = "0123456789abcdef0123";
        assert!(Replay::default().check(Some(now), Some(nonce), now).is_err());
        let mut replay = Replay::memory();
        replay.db.as_ref().unwrap().pragma_update(None, "query_only", true).unwrap();
        assert!(replay.check(Some(now), Some(nonce), now).is_err());
        let count: i64 = replay.db.as_ref().unwrap().query_row("SELECT COUNT(*) FROM receipts", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
        replay.db.as_ref().unwrap().pragma_update(None, "query_only", false).unwrap();
        assert!(replay.check(Some(now), Some(nonce), now).is_ok(), "failed durable admission must not run or consume a receipt in memory");
        replay.db.as_ref().unwrap().execute_batch("CREATE TRIGGER reject_receipt BEFORE INSERT ON receipts BEGIN SELECT RAISE(ABORT,'simulated write failure'); END;").unwrap();
        assert!(replay.check(Some(now + WINDOW_MS + 1), Some("new-nonce-0000000001"), now + WINDOW_MS + 1).is_err());
        let count: i64 = replay.db.as_ref().unwrap().query_row("SELECT COUNT(*) FROM receipts", [], |row| row.get(0)).unwrap();
        let last_ms: i64 = replay.db.as_ref().unwrap().query_row("SELECT last_ms FROM replay_clock WHERE id=1", [], |row| row.get(0)).unwrap();
        assert_eq!((count, last_ms), (1, now), "a failed insert must roll back pruning and the clock watermark too");
        let folder = Folder::new();
        std::fs::write(folder.0.join("connect-replay"), b"not a directory").unwrap();
        assert!(Replay::open(&folder.0).is_err());
        let folder = Folder::new();
        drop(Replay::open(&folder.0).unwrap());
        let path = folder.0.join("connect-replay/receipts.sqlite");
        std::fs::write(&path, b"").unwrap();
        assert!(Replay::open(&folder.0).is_err(), "a truncated ledger must not become a fresh one");
        std::fs::remove_file(&path).unwrap();
        assert!(Replay::open(&folder.0).is_err(), "a missing ledger in an existing directory must not reset admission history");
    }

    #[test]
    fn capacity_never_evicts_live_receipts_and_only_expired_rows_are_removed() {
        let mut replay = Replay::memory();
        let now = 1_700_000_000_000;
        replay.db.as_ref().unwrap().execute(
            "WITH RECURSIVE rows(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM rows WHERE n<20000)
             INSERT INTO receipts(nonce_hash,ts) SELECT CAST(printf('%032d',n) AS BLOB),?1 FROM rows", [now],
        ).unwrap();
        assert!(replay.check(Some(now), Some("fresh-nonce-00000001"), now).is_err());
        let count: i64 = replay.db.as_ref().unwrap().query_row("SELECT COUNT(*) FROM receipts", [], |row| row.get(0)).unwrap();
        assert_eq!(count, MAX_SEEN);
        assert!(replay.check(Some(now + WINDOW_MS), Some("fresh-nonce-00000001"), now + WINDOW_MS).is_err(), "boundary receipts are still live");
        assert!(replay.check(Some(now + WINDOW_MS + 1), Some("fresh-nonce-00000001"), now + WINDOW_MS + 1).is_ok());
        let count: i64 = replay.db.as_ref().unwrap().query_row("SELECT COUNT(*) FROM receipts", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn clock_rollback_after_restart_cannot_revive_pruned_ciphertext() {
        let folder = Folder::new();
        let now = 1_700_000_000_000;
        let old_nonce = "0123456789abcdef0123";
        {
            let mut replay = Replay::open(&folder.0).unwrap();
            assert!(replay.check(Some(now), Some(old_nonce), now).is_ok());
            assert!(replay.check(Some(now + WINDOW_MS + 1), Some("later-nonce-00000001"), now + WINDOW_MS + 1).is_ok());
            let count: i64 = replay.db.as_ref().unwrap().query_row("SELECT COUNT(*) FROM receipts", [], |row| row.get(0)).unwrap();
            assert_eq!(count, 1);
        }
        let mut restarted = Replay::open(&folder.0).unwrap();
        assert!(restarted.check(Some(now), Some(old_nonce), now).is_err());
        assert!(restarted.check(Some(now + WINDOW_MS + 2), Some("current-nonce-0000001"), now + WINDOW_MS + 2).is_ok());
    }
}
