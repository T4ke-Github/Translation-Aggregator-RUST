//! Secret storage backed by Windows Credential Manager.
//!
//! Secrets are written only through the OS credential store. They are never
//! written to settings files, logs, or error messages. Reading a secret is
//! only for the request path; the settings and Test views must mask it.

use std::io;

/// Prefix for all credential target names owned by this application.
pub const CREDENTIAL_PREFIX: &str = "Translation Aggregator/";

/// Maximum length of a secret in bytes, matching CredWrite's blob limit.
const MAX_SECRET_BYTES: usize = 256 * 1024;

#[cfg(windows)]
const FILETIME_ZERO: windows_sys::Win32::Foundation::FILETIME =
    windows_sys::Win32::Foundation::FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };

/// Build and validate a credential target name.
pub fn credential_name(name: &str) -> Result<String, io::Error> {
    if name.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential name must not be empty",
        ));
    }
    if name.len() > 256 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential name is too long",
        ));
    }
    if name.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential name must not contain NUL",
        ));
    }
    if name.contains('/') || name.contains('\\') || name.contains(':') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential name must not contain path separators",
        ));
    }
    if name.chars().any(|ch| ch.is_control()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "credential name must not contain control characters",
        ));
    }
    Ok(format!("{CREDENTIAL_PREFIX}{name}"))
}

fn secret_bytes(secret: &str) -> Result<&[u8], io::Error> {
    let bytes = secret.as_bytes();
    if bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "secret must not be empty",
        ));
    }
    if bytes.len() > MAX_SECRET_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "secret is too long",
        ));
    }
    Ok(bytes)
}

/// Save a secret in the OS credential store, replacing any existing value.
pub fn save_secret(name: &str, secret: &str) -> io::Result<()> {
    let target = credential_name(name)?;
    let bytes = secret_bytes(secret)?;
    save_secret_win32(&target, bytes)
}

/// Read a secret from the OS credential store.
pub fn read_secret(name: &str) -> io::Result<Option<String>> {
    let target = credential_name(name)?;
    read_secret_win32(&target)
}

/// Delete a secret from the OS credential store. Missing secrets are ignored.
pub fn delete_secret(name: &str) -> io::Result<()> {
    let target = credential_name(name)?;
    delete_secret_win32(&target)
}

#[cfg(windows)]
mod win32 {
    use super::MAX_SECRET_BYTES;
    use std::io;
    use std::ptr;
    use windows_sys::Win32::Foundation::{ERROR_NOT_FOUND, GetLastError};
    use windows_sys::Win32::Security::Credentials::{
        CRED_TYPE_GENERIC, CREDENTIALW, CredDeleteW, CredFree, CredReadW, CredWriteW,
    };

    fn last_error() -> io::Error {
        // SAFETY: GetLastError has no preconditions.
        let code = unsafe { GetLastError() };
        io::Error::from_raw_os_error(code as i32)
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(super) fn save(credential: &CREDENTIALW) -> io::Result<()> {
        // SAFETY: CredWriteW reads the fully initialized CREDENTIALW and its
        // NUL-terminated pointer fields for the duration of the call.
        let ok = unsafe { CredWriteW(credential, 0) };
        if ok == 0 {
            return Err(last_error());
        }
        Ok(())
    }

    pub(super) fn read(target: &str) -> io::Result<Option<String>> {
        let target_name = wide(target);
        let mut credential = ptr::null_mut();
        // SAFETY: target_name is NUL terminated and outlives the call. The
        // returned pointer is freed with CredFree below.
        let ok = unsafe { CredReadW(target_name.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) };
        if ok == 0 {
            // SAFETY: GetLastError has no preconditions.
            let code = unsafe { GetLastError() };
            if code == ERROR_NOT_FOUND {
                return Ok(None);
            }
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        if credential.is_null() {
            return Ok(None);
        }
        // SAFETY: CredReadW returned this credential pointer; CredFree is the
        // matching allocator.
        let _guard = CredFreeGuard(credential);
        // SAFETY: CredReadW returned a valid credential pointer.
        let bytes = unsafe { (*credential).CredentialBlobSize } as usize;
        if bytes == 0 {
            return Ok(None);
        }
        if bytes > MAX_SECRET_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "credential blob is too long",
            ));
        }
        // SAFETY: CredReadW returned a valid credential pointer and size.
        let blob = unsafe { (*credential).CredentialBlob };
        if blob.is_null() {
            return Ok(None);
        }
        // SAFETY: CredentialBlob and CredentialBlobSize describe this blob.
        let slice = unsafe { std::slice::from_raw_parts(blob, bytes) };
        Ok(Some(String::from_utf8_lossy(slice).into_owned()))
    }

    pub(super) fn delete(target: &str) -> io::Result<()> {
        let target_name = wide(target);
        // SAFETY: target_name is NUL terminated and outlives the call.
        let ok = unsafe { CredDeleteW(target_name.as_ptr(), CRED_TYPE_GENERIC, 0) };
        if ok == 0 {
            // SAFETY: GetLastError has no preconditions.
            let code = unsafe { GetLastError() };
            if code == ERROR_NOT_FOUND {
                return Ok(());
            }
            return Err(io::Error::from_raw_os_error(code as i32));
        }
        Ok(())
    }

    struct CredFreeGuard(*mut CREDENTIALW);
    impl Drop for CredFreeGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: this pointer was returned by CredReadW and has not
                // been freed yet.
                unsafe { CredFree(self.0.cast()) };
            }
        }
    }
}

#[cfg(windows)]
fn save_secret_win32(target: &str, secret: &[u8]) -> io::Result<()> {
    use std::ptr;
    use windows_sys::Win32::Security::Credentials::{
        CRED_PERSIST_ENTERPRISE, CRED_TYPE_GENERIC, CREDENTIALW,
    };

    let mut target_name = target
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let user_name =
        std::env::var("USERNAME").unwrap_or_else(|_| "Translation Aggregator".to_owned());
    let mut user_name = user_name
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let credential = CREDENTIALW {
        Flags: 0,
        Type: CRED_TYPE_GENERIC,
        TargetName: target_name.as_mut_ptr(),
        Comment: ptr::null_mut(),
        LastWritten: FILETIME_ZERO,
        CredentialBlobSize: secret.len() as u32,
        CredentialBlob: secret.as_ptr().cast_mut(),
        Persist: CRED_PERSIST_ENTERPRISE,
        AttributeCount: 0,
        Attributes: ptr::null_mut(),
        TargetAlias: ptr::null_mut(),
        UserName: user_name.as_mut_ptr(),
    };
    win32::save(&credential)
}

#[cfg(windows)]
fn read_secret_win32(target: &str) -> io::Result<Option<String>> {
    win32::read(target)
}

#[cfg(windows)]
fn delete_secret_win32(target: &str) -> io::Result<()> {
    win32::delete(target)
}

#[cfg(not(windows))]
fn save_secret_win32(_target: &str, _secret: &[u8]) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "credential store requires Windows",
    ))
}

#[cfg(not(windows))]
fn read_secret_win32(_target: &str) -> io::Result<Option<String>> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "credential store requires Windows",
    ))
}

#[cfg(not(windows))]
fn delete_secret_win32(_target: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "credential store requires Windows",
    ))
}

#[cfg(test)]
mod tests {
    use super::credential_name;

    #[test]
    fn valid_names_get_application_prefix() {
        assert_eq!(
            credential_name("api-key").unwrap(),
            "Translation Aggregator/api-key"
        );
    }

    #[test]
    fn bad_names_are_rejected_without_their_contents() {
        for name in ["", "a/b", "a\\b", "a:b", "a\0b", "a\nb"] {
            let error = credential_name(name).unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
            if !name.is_empty() {
                assert!(!error.to_string().contains(name));
            }
        }
    }

    #[test]
    fn long_names_are_rejected() {
        let name = "a".repeat(257);
        assert!(credential_name(&name).is_err());
    }

    #[test]
    fn empty_and_oversized_secrets_are_rejected() {
        assert_eq!(
            super::secret_bytes("").unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        let secret = "a".repeat(super::MAX_SECRET_BYTES + 1);
        assert_eq!(
            super::secret_bytes(&secret).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
}
