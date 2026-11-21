//! Platform foundations for settings storage.
//!
//! These helpers resolve settings without consulting the process working directory and save
//! complete byte payloads via a same-directory temporary file.

pub mod clipboard;
pub mod credentials;
pub mod history_storage;
pub mod settings_queue;

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const APP_DIR: &str = "Translation Aggregator";

/// Parsed settings text, retaining the original bytes for lossless edits.
#[derive(Debug, Clone)]
pub struct SettingsDocument {
    source: String,
    value: toml::Value,
    version: u64,
    writable: bool,
    warnings: Vec<String>,
}

impl SettingsDocument {
    /// Parse and validate a settings TOML document without normalizing its formatting.
    pub fn parse(source: &str) -> Result<Self, String> {
        let value: toml::Value = toml::from_str(source).map_err(|e: toml::de::Error| {
            let (line, column) = e
                .span()
                .map(|span| {
                    source[..span.start.min(source.len())].chars().fold(
                        (1, 1),
                        |(line, column), ch| {
                            if ch == '\n' {
                                (line + 1, 1)
                            } else {
                                (line, column + 1)
                            }
                        },
                    )
                })
                .unwrap_or((1, 1));
            format!("invalid TOML at line {line}, column {column}")
        })?;
        let mut warnings = Vec::new();
        let version = match value.get("schema_version") {
            None => {
                warnings.push("schema_version missing; assumed v1".to_owned());
                1
            }
            Some(v) => v
                .as_integer()
                .and_then(|n| u64::try_from(n).ok())
                .filter(|n| *n > 0)
                .ok_or_else(|| "schema_version must be a positive integer".to_owned())?,
        };
        let writable = version <= 1;
        if !writable {
            warnings.push(format!(
                "settings schema v{version} is newer; file is read-only"
            ));
        }
        Ok(Self {
            source: source.to_owned(),
            value,
            version,
            writable,
            warnings,
        })
    }

    /// Save this document safely, refusing to replace malformed or newer settings files.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if !self.writable {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "newer settings schema is read-only",
            ));
        }
        if path.exists() {
            let current = fs::read(path)?;
            let current = parse_settings_bytes(&current)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            if !current.is_writable() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "existing settings use a newer schema",
                ));
            }
        }
        let mut source = self.source.clone();
        if self.version == 1 {
            let mut document: toml_edit::DocumentMut = source
                .parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid settings TOML"))?;
            document.remove("schema_version");
            source = document.to_string();
            source = format!("schema_version = 1\n{source}");
        }
        atomic_save(path, source.as_bytes())
    }

    /// Return the parsed settings schema version.
    pub fn version(&self) -> u64 {
        self.version
    }
    /// Whether this version is safe for the current app to edit.
    pub fn is_writable(&self) -> bool {
        self.writable
    }
    /// Return non-fatal diagnostics found while parsing.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Parsed TOML, including settings understood by other parts of the application.
    pub fn value(&self) -> &toml::Value {
        &self.value
    }
}

impl std::fmt::Display for SettingsDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.source)
    }
}

/// Loaded settings and whether the backup was used.
#[derive(Debug, Clone)]
pub struct LoadedSettings {
    /// The parsed settings document.
    pub document: SettingsDocument,
    /// Whether the primary file failed to parse and a backup supplied the data.
    pub recovered_from_backup: bool,
}

/// Load settings, recovering from corrupt files without discarding their bytes.
pub fn load_settings(path: &Path) -> io::Result<LoadedSettings> {
    let primary_error = match fs::read(path) {
        Ok(bytes) => match parse_settings_bytes(&bytes) {
            Ok(document) => {
                return Ok(LoadedSettings {
                    document,
                    recovered_from_backup: false,
                });
            }
            Err(error) => error,
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(empty_settings()),
        Err(error) => return Err(error),
    };
    if let Ok(bytes) = fs::read(backup_path(path))
        && let Ok(mut document) = parse_settings_bytes(&bytes)
    {
        quarantine_corrupt_primary(path)?;
        document.warnings.push(format!(
            "{}: {primary_error}; recovered from backup",
            path.display()
        ));
        return Ok(LoadedSettings {
            document,
            recovered_from_backup: true,
        });
    }
    let mut loaded = empty_settings();
    loaded.document.warnings.push(format!(
        "{}: {primary_error}; backup unavailable or invalid; using defaults",
        path.display()
    ));
    Ok(loaded)
}

fn empty_settings() -> LoadedSettings {
    LoadedSettings {
        document: SettingsDocument::parse("").expect("empty TOML is valid"),
        recovered_from_backup: false,
    }
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        "{}.bak",
        path.file_name().unwrap_or_default().to_string_lossy()
    ))
}

fn parse_settings_bytes(bytes: &[u8]) -> Result<SettingsDocument, String> {
    if bytes.is_empty() {
        return Err("settings file is empty".to_owned());
    }
    if bytes.contains(&0) {
        return Err("settings file contains NUL bytes".to_owned());
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| "settings file is not valid UTF-8".to_owned())?;
    SettingsDocument::parse(text)
}

fn quarantine_corrupt_primary(path: &Path) -> io::Result<()> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let stem = name.strip_suffix(".toml").unwrap_or(&name);
    let corrupt = path.with_file_name(format!("{stem}.corrupt-{stamp}.toml"));
    fs::rename(path, corrupt)
}

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
                replace_backup(&backup_temp, &backup)
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

fn replace_backup(staged: &Path, backup: &Path) -> io::Result<()> {
    #[cfg(windows)]
    if backup.exists() {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;

        let old: Vec<u16> = backup.as_os_str().encode_wide().chain(Some(0)).collect();
        let new: Vec<u16> = staged.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: both UTF-16 paths are NUL-terminated and live through the call;
        // the optional backup and reserved arguments are null as allowed by Win32.
        if unsafe {
            ReplaceFileW(
                old.as_ptr(),
                new.as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                std::ptr::null(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        return Ok(());
    }
    fs::rename(staged, backup)
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
        let id = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let p =
            std::env::temp_dir().join(format!("ta-platform-{}-{stamp}-{id}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn save_does_not_remove_nested_schema_version() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        let source = "[future]\nschema_version = 7 # keep nested key\nother = true\n";
        SettingsDocument::parse(source)
            .unwrap()
            .save(&path)
            .unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.starts_with("schema_version = 1\n"));
        assert!(saved.contains("schema_version = 7 # keep nested key"));
        assert!(saved.contains("other = true"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn versioned_document_preserves_unknown_values_and_unchanged_formatting() {
        let source = "# keep me\n[future]
answer = 42 # comment\n";
        let parsed = SettingsDocument::parse(source).unwrap();
        assert_eq!(parsed.version(), 1);
        assert_eq!(parsed.to_string(), source);
        assert!(parsed.is_writable());
    }

    #[test]
    fn missing_version_is_v1_with_warning_and_newer_is_read_only() {
        let legacy = SettingsDocument::parse("value = 3\n").unwrap();
        assert_eq!(legacy.version(), 1);
        assert!(legacy.warnings().iter().any(|w| w.contains("assumed v1")));
        let newer = SettingsDocument::parse("schema_version = 99\n").unwrap();
        assert!(!newer.is_writable());
    }

    #[test]
    fn invalid_schema_versions_are_rejected() {
        for source in [
            "schema_version = 0\n",
            "schema_version = -1\n",
            "schema_version = \"v1\"\n",
        ] {
            assert!(SettingsDocument::parse(source).is_err());
        }
    }

    #[test]
    fn malformed_toml_diagnostic_has_location_without_source_text() {
        let marker = "private_marker_7f31";
        let source = format!("ok = 1\nsecret = [{marker}\n");
        let error = SettingsDocument::parse(&source).unwrap_err();
        assert!(error.contains("line 2"), "{error}");
        assert!(error.contains("column"), "{error}");
        assert!(!error.contains(marker), "{error}");
    }

    #[test]
    fn corrupt_settings_warning_has_path_location_and_no_source_text() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        let marker = "private_marker_7f31";
        fs::write(&path, format!("ok = 1\nsecret = [{marker}\n")).unwrap();
        let loaded = load_settings(&path).unwrap();
        let warning = loaded.document.warnings().join(" ");
        assert!(warning.contains(path.to_str().unwrap()), "{warning}");
        assert!(warning.contains("line 2"), "{warning}");
        assert!(warning.contains("column"), "{warning}");
        assert!(warning.contains("using defaults"), "{warning}");
        assert!(!warning.contains(marker), "{warning}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_puts_version_first_and_retains_unknown_table_comments() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        let doc = SettingsDocument::parse(
            "title = \"kept\" # note\n[future]\nanswer = 42 # future note\n",
        )
        .unwrap();
        doc.save(&path).unwrap();
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.starts_with("schema_version = 1\n"));
        assert!(saved.contains("title = \"kept\" # note"));
        assert!(saved.contains("[future]\nanswer = 42 # future note"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_moves_late_version_to_first_key() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        SettingsDocument::parse("value = 3\nschema_version = 1\n")
            .unwrap()
            .save(&path)
            .unwrap();
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .starts_with("schema_version = 1\n")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_save_refuses_future_and_corrupt_files_without_changing_bytes() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        let future = b"schema_version = 9\nvalue = 1\n";
        fs::write(&path, future).unwrap();
        let result = SettingsDocument::parse("value = 2\n").unwrap().save(&path);
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), future);
        fs::write(&path, b"invalid = [\n").unwrap();
        let corrupt = fs::read(&path).unwrap();
        assert!(
            SettingsDocument::parse("value = 2\n")
                .unwrap()
                .save(&path)
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), corrupt);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn absent_settings_start_with_empty_v1_document() {
        let dir = temp_dir();
        let loaded = load_settings(&dir.join("settings.toml")).unwrap();
        assert_eq!(loaded.document.version(), 1);
        assert!(!loaded.recovered_from_backup);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_primary_recovers_backup_without_overwriting_corrupt_file() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        fs::write(&path, "invalid = [\n").unwrap();
        fs::write(dir.join("settings.toml.bak"), "schema_version = 1\n").unwrap();
        let recovered = load_settings(&path).unwrap();
        assert!(recovered.recovered_from_backup);
        assert!(!path.exists());
        assert!(
            fs::read_dir(&dir)
                .unwrap()
                .filter_map(Result::ok)
                .any(|entry| {
                    entry
                        .path()
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("settings.corrupt-")
                })
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_primary_recovers_backup_and_quarantines_primary() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        let corrupt = b"invalid = [\n";
        fs::write(&path, corrupt).unwrap();
        fs::write(dir.join("settings.toml.bak"), "schema_version = 1\n").unwrap();

        let loaded = load_settings(&path).unwrap();
        assert!(loaded.recovered_from_backup);
        assert!(
            loaded
                .document
                .warnings()
                .iter()
                .any(|warning| warning.contains("recovered from backup"))
        );
        assert!(!path.exists());
        let quarantined: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|candidate| {
                candidate
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("settings.corrupt-")
                    && candidate.extension().is_some_and(|ext| ext == "toml")
            })
            .collect();
        assert_eq!(quarantined.len(), 1);
        assert_eq!(fs::read(&quarantined[0]).unwrap(), corrupt);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_primary_and_backup_use_empty_defaults_and_preserve_both() {
        let dir = temp_dir();
        let path = dir.join("settings.toml");
        let primary = b"broken = [\n";
        let backup = b"also = [\n";
        fs::write(&path, primary).unwrap();
        fs::write(dir.join("settings.toml.bak"), backup).unwrap();

        let loaded = load_settings(&path).unwrap();
        assert!(!loaded.recovered_from_backup);
        assert_eq!(loaded.document.version(), 1);
        assert_eq!(loaded.document.to_string(), "");
        assert!(
            loaded
                .document
                .warnings()
                .iter()
                .any(|warning| warning.contains("using defaults"))
        );
        assert_eq!(fs::read(&path).unwrap(), primary);
        assert_eq!(fs::read(dir.join("settings.toml.bak")).unwrap(), backup);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn empty_nul_and_non_utf8_primary_recover_valid_backup() {
        for (index, invalid) in [Vec::new(), vec![0, 1], vec![0xff, 0xfe]]
            .into_iter()
            .enumerate()
        {
            let dir = temp_dir();
            let path = dir.join("settings.toml");
            fs::write(&path, &invalid).unwrap();
            fs::write(dir.join("settings.toml.bak"), "schema_version = 1\n").unwrap();
            let loaded = load_settings(&path).unwrap_or_else(|e| panic!("case {index}: {e}"));
            assert!(loaded.recovered_from_backup, "case {index}");
            let quarantined = fs::read_dir(&dir)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|candidate| {
                    candidate
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("settings.corrupt-")
                })
                .expect("corrupt primary quarantined");
            assert_eq!(fs::read(quarantined).unwrap(), invalid);
            fs::remove_dir_all(dir).unwrap();
        }
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
        atomic_save(&path, b"newest complete").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"newest complete");
        assert_eq!(
            fs::read(dir.join("settings.toml.bak")).unwrap(),
            b"new complete"
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
