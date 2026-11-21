//! Coalesce settings updates and write them off the caller thread.

use crate::SettingsDocument;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const DEBOUNCE: Duration = Duration::from_secs(1);
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Default)]
struct State {
    pending: Option<(SettingsDocument, Instant, u64)>,
    latest: u64,
    completed: u64,
    flush_requested: bool,
    stopping: bool,
    finished: bool,
    last_error: Option<io::Error>,
}

type Shared = Arc<(Mutex<State>, Condvar)>;

/// One background worker with a single replaceable pending settings document.
pub struct SettingsSaveQueue {
    shared: Shared,
    worker: Option<JoinHandle<()>>,
}

impl SettingsSaveQueue {
    /// Start a worker that writes settings at `path`.
    pub fn new(path: impl AsRef<Path>) -> io::Result<Self> {
        let path = path.as_ref().to_owned();
        let shared: Shared = Arc::new((Mutex::new(State::default()), Condvar::new()));
        let worker_state = Arc::clone(&shared);
        let worker = thread::Builder::new()
            .name("settings-save".into())
            .spawn(move || run_worker(path, worker_state))?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }

    /// Replace the pending document without blocking on filesystem I/O.
    pub fn queue(&self, document: SettingsDocument) -> io::Result<()> {
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().map_err(|_| worker_stopped())?;
        if state.stopping || state.finished {
            return Err(worker_stopped());
        }
        state.latest = state
            .latest
            .checked_add(1)
            .ok_or_else(|| io::Error::other("settings generation exhausted"))?;
        state.pending = Some((document, Instant::now(), state.latest));
        wake.notify_one();
        Ok(())
    }

    /// Save the most recent queued document now, waiting at most two seconds.
    pub fn flush(&self) -> io::Result<()> {
        self.wait_for_save(false)
    }

    /// Save pending changes and stop, waiting at most two seconds for the worker.
    /// If this times out, the caller must decide whether the process can exit.
    pub fn shutdown(&mut self) -> io::Result<()> {
        if self.worker.is_none() {
            return Ok(());
        }
        let result = self.wait_for_save(true);
        if let Some(worker) = self.worker.take()
            && worker.is_finished()
        {
            let _ = worker.join();
        }
        result
    }

    fn wait_for_save(&self, stop: bool) -> io::Result<()> {
        let limit = Instant::now() + SHUTDOWN_TIMEOUT;
        let (lock, wake) = &*self.shared;
        let mut state = lock.lock().map_err(|_| worker_stopped())?;
        if state.finished && !stop {
            return Err(worker_stopped());
        }
        if stop {
            state.stopping = true;
        } else if state.stopping {
            return Err(worker_stopped());
        }
        let target = state.latest;
        state.flush_requested = true;
        wake.notify_one();
        while state.completed < target || (stop && !state.finished) {
            let remaining = limit.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "settings save did not complete within two seconds",
                ));
            }
            let (next, _) = wake
                .wait_timeout(state, remaining)
                .map_err(|_| worker_stopped())?;
            state = next;
        }
        state.last_error.take().map_or(Ok(()), Err)
    }
}

impl Drop for SettingsSaveQueue {
    fn drop(&mut self) {
        if self.worker.is_some() {
            let _ = self.shutdown();
        }
    }
}

fn worker_stopped() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "settings worker stopped")
}

fn deadline(last_change: Instant) -> Instant {
    last_change + DEBOUNCE
}

fn due(now: Instant, last_change: Instant) -> bool {
    now >= deadline(last_change)
}

fn run_worker(path: PathBuf, shared: Shared) {
    let (lock, wake) = &*shared;
    loop {
        let (document, generation) = {
            let mut state = lock.lock().expect("settings state poisoned");
            loop {
                if let Some((_, changed, _)) = state.pending.as_ref() {
                    if state.flush_requested || due(Instant::now(), *changed) {
                        break;
                    }
                    let remaining = deadline(*changed).saturating_duration_since(Instant::now());
                    state = wake
                        .wait_timeout(state, remaining)
                        .expect("settings state poisoned")
                        .0;
                } else if state.stopping {
                    state.finished = true;
                    wake.notify_all();
                    return;
                } else {
                    state.flush_requested = false;
                    state = wake.wait(state).expect("settings state poisoned");
                }
            }
            let (document, _, generation) = state.pending.take().expect("pending document");
            state.flush_requested = false;
            (document, generation)
        };
        let result = document.save(&path);
        let mut state = lock.lock().expect("settings state poisoned");
        state.completed = generation;
        state.last_error = result.err();
        wake.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TEMP_SEQUENCE;
    use std::fs;

    fn temp_dir() -> PathBuf {
        let id = TEMP_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ta-queue-{}-{id}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn debounce_is_clock_driven_and_resets_on_each_change() {
        let start = Instant::now();
        let second = start + Duration::from_millis(900);
        assert!(!due(second, start));
        assert!(!due(start + Duration::from_millis(1_899), second));
        assert!(due(start + Duration::from_millis(1_900), second));
    }

    #[test]
    fn shutdown_writes_latest_document_atomically() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        let mut queue = SettingsSaveQueue::new(&path).unwrap();
        for value in 0..50 {
            queue
                .queue(SettingsDocument::parse(&format!("value = {value}\n")).unwrap())
                .unwrap();
        }
        queue.shutdown().unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("value = 49"));
        assert!(!dir.join("settings.toml.bak").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn flush_reports_save_errors_without_blocking_a_later_save() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        fs::create_dir(&path).unwrap();
        let mut queue = SettingsSaveQueue::new(&path).unwrap();
        queue
            .queue(SettingsDocument::parse("value = 1\n").unwrap())
            .unwrap();
        assert!(queue.flush().is_err());
        fs::remove_dir(&path).unwrap();
        queue
            .queue(SettingsDocument::parse("value = 2\n").unwrap())
            .unwrap();
        queue.shutdown().unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains("value = 2"));
        fs::remove_dir_all(dir).unwrap();
    }
}
