//! Platform foundations for settings storage.
//!
//! These helpers resolve settings without consulting the process working directory and save
//! complete byte payloads via a same-directory temporary file.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const APP_DIR: &str = "Translation Aggregator";
const SETTINGS_FILE: &str = "settings.toml";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Paths for user settings and rebuildable application data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsPaths {
    /// Full path to `settings.toml`.
    pub settings_file: PathBuf,
    /// Directory for rebuildable application data.
    pub data_dir: PathBuf,
}

/// Resolve settings and data paths. `portable` is detected beside the executable.
pub fn resolve_settings_paths(
    executable: &Path,
    appdata: Option<&Path>,
    local_appdata: Option<&Path>,
) -> io::Result<SettingsPaths> {
    let exe_dir = executable.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "executable has no parent directory",
        )
    })?;
    if exe_dir.join("portable").exists() {
        let data_dir = exe_dir.join("data");
        return Ok(SettingsPaths {
            settings_file: data_dir.join(SETTINGS_FILE),
            data_dir,
        });
    }
    let appdata =
        appdata.ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA is not set"))?;
    let local_appdata = local_appdata
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "LOCALAPPDATA is not set"))?;
    Ok(SettingsPaths {
        settings_file: appdata.join(APP_DIR).join(SETTINGS_FILE),
        data_dir: local_appdata.join(APP_DIR),
    })
}

/// Resolve using the current executable and Windows user data environment variables.
pub fn settings_paths() -> io::Result<SettingsPaths> {
    let executable = std::env::current_exe()?;
    let appdata = std::env::var_os("APPDATA").map(PathBuf::from);
    let local_appdata = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    resolve_settings_paths(&executable, appdata.as_deref(), local_appdata.as_deref())
}

/// Atomically replace `path` with `contents`, preserving the former file as `.bak`.
/// If backup or replacement fails, the original destination remains untouched.
pub fn atomic_save(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no filename"))?;
    let mut temp = None;
    for _ in 0..100 {
        let id = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{}.{}.{}.tmp",
            name.to_string_lossy(),
            std::process::id(),
            id
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temp = Some((candidate, file));
                break;
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    let (temp_path, mut file) = temp.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate temporary file",
        )
    })?;
    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        if path.exists() {
            // Copy the old file first so the destination stays present until the final rename.
            let backup = path.with_file_name(format!("{}.bak", name.to_string_lossy()));
            let backup_temp = parent.join(format!(
                ".{}.{}.{}.bak.tmp",
                name.to_string_lossy(),
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let backup_result = (|| {
                let mut source = File::open(path)?;
                let mut dest = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&backup_temp)?;
                io::copy(&mut source, &mut dest)?;
                dest.sync_all()?;
                drop(dest);
                if backup.exists() {
                    fs::remove_file(&backup)?;
                }
                fs::rename(&backup_temp, &backup)
            })();
            if backup_result.is_err() {
                let _ = fs::remove_file(&backup_temp);
                return backup_result;
            }
        }
        fs::rename(&temp_path, path)?;
        if let Ok(dir) = File::open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("ta-platform-{stamp}"));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn normal_paths_use_roaming_and_local_appdata_not_cwd() {
        let exe = Path::new(r"C:\Program Files\TA\ta.exe");
        let paths = resolve_settings_paths(
            exe,
            Some(Path::new(r"C:\Users\u\Roaming")),
            Some(Path::new(r"C:\Users\u\Local")),
        )
        .unwrap();
        assert_eq!(
            paths.settings_file,
            Path::new(r"C:\Users\u\Roaming")
                .join(APP_DIR)
                .join(SETTINGS_FILE)
        );
        assert_eq!(paths.data_dir, Path::new(r"C:\Users\u\Local").join(APP_DIR));
    }

    #[test]
    fn portable_marker_places_settings_and_data_beside_executable() {
        let dir = temp_dir();
        fs::write(dir.join("portable"), b"").unwrap();
        let paths = resolve_settings_paths(&dir.join("app.exe"), None, None).unwrap();
        assert_eq!(paths.settings_file, dir.join("data/settings.toml"));
        assert_eq!(paths.data_dir, dir.join("data"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_replaces_whole_file_and_keeps_previous_backup() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        atomic_save(&path, b"old complete").unwrap();
        atomic_save(&path, b"new complete").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new complete");
        assert_eq!(
            fs::read(dir.join("settings.toml.bak")).unwrap(),
            b"old complete"
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_backup_preserves_original() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        fs::write(&path, b"original").unwrap();
        fs::create_dir(path.with_extension("toml.bak")).unwrap();
        assert!(atomic_save(&path, b"replacement").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        fs::remove_dir_all(dir).unwrap();
    }
}
