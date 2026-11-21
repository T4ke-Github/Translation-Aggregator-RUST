use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};
use ta_core::{DEFAULT_HISTORY_CAPACITY, History};
use ta_platform::history_storage::{load_history, save_history};

pub(super) struct HistoryDisk {
    path: PathBuf,
    _lock: File,
}

impl HistoryDisk {
    pub(super) fn open(dir: &Path, keep: bool) -> io::Result<(History, Option<Self>)> {
        let path = dir.join("history.json");
        if !keep && !path.try_exists()? && !dir.join("history.json.bak").try_exists()? {
            return Ok((History::default(), None));
        }
        fs::create_dir_all(dir)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(dir.join("history.lock"))?;
        lock.try_lock().map_err(io::Error::from)?;
        let history = load_history(&path, keep, DEFAULT_HISTORY_CAPACITY)?;
        Ok((history, keep.then_some(Self { path, _lock: lock })))
    }

    pub(super) fn save(&self, history: &History) -> io::Result<()> {
        save_history(&self.path, history, true)
    }
}
