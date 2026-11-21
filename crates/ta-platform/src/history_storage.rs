//! Optional on-disk history storage.

use std::fs;
use std::io;
use std::path::Path;
use ta_core::{History, decode_history_snapshot, encode_history_snapshot};

const MAX_HISTORY_BYTES: u64 = 64 * 1024 * 1024;

fn remove_if_present(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

fn forget_history(path: &Path) -> io::Result<()> {
    remove_if_present(path)?;
    remove_if_present(&super::backup_path(path))
}

/// Save only when history retention is enabled; disabling removes data and backup.
pub fn save_history(path: &Path, history: &History, keep_between_sessions: bool) -> io::Result<()> {
    if !keep_between_sessions {
        return forget_history(path);
    }
    let snapshot = encode_history_snapshot(history).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "history snapshot could not be encoded",
        )
    })?;
    if snapshot.len() as u64 > MAX_HISTORY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history snapshot too large",
        ));
    }
    super::atomic_save(path, snapshot.as_bytes())
}

/// Restore an opted-in history, or discard old files if retention is disabled.
pub fn load_history(
    path: &Path,
    keep_between_sessions: bool,
    capacity: usize,
) -> io::Result<History> {
    if !keep_between_sessions {
        forget_history(path)?;
        return Ok(History::with_capacity(capacity));
    }
    let size = match fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(History::with_capacity(capacity));
        }
        Err(error) => return Err(error),
    };
    if size > MAX_HISTORY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history file too large",
        ));
    }
    let bytes = fs::read(path)?;
    if bytes.len() as u64 > MAX_HISTORY_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history file too large",
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "history file is not UTF-8"))?;
    decode_history_snapshot(text, capacity)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "history snapshot is invalid"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ta_core::{ContextId, HistoryEntry, HistoryEntryId};

    fn temp_dir() -> std::path::PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ta-history-{}-{stamp}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn history_defaults_to_memory_and_opt_in_roundtrips_then_forgets_disk() {
        let dir = temp_dir();
        let path = dir.join("history.json");
        let mut history = ta_core::History::with_capacity(2);
        history.push(HistoryEntry::new(
            HistoryEntryId::new(),
            std::time::SystemTime::UNIX_EPOCH,
            ContextId::new("typed").unwrap(),
            "source",
            "processed",
        ));
        save_history(&path, &history, false).unwrap();
        assert!(!path.exists());
        save_history(&path, &history, true).unwrap();
        assert!(path.exists());
        let restored = load_history(&path, true, 2).unwrap();
        assert_eq!(restored.iter().next().unwrap().original_text, "source");
        save_history(&path, &history, true).unwrap();
        assert!(dir.join("history.json.bak").exists());
        save_history(&path, &history, false).unwrap();
        assert!(!path.exists());
        assert!(!dir.join("history.json.bak").exists());
        assert!(load_history(&path, false, 2).unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupted_history_is_not_restored_or_exposed() {
        let dir = temp_dir();
        let path = dir.join("history.json");
        std::fs::write(&path, b"{private_key_123}").unwrap();
        let error = load_history(&path, true, 2).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(!error.to_string().contains("private_key_123"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
