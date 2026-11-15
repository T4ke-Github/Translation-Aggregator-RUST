//! Clipboard text helpers and the Win32 clipboard listener/read wrapper.

use std::time::Duration;

/// Convert Windows UTF-16 clipboard data, replacing malformed surrogate sequences.
pub fn decode_utf16(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

#[cfg(any(windows, test))]
fn decode_nul_terminated(units: &[u16]) -> std::io::Result<String> {
    let end = units.iter().position(|&unit| unit == 0).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "clipboard text lacks terminator",
        )
    })?;
    Ok(decode_utf16(&units[..end]))
}

/// Whether monitor processing must skip clipboard data due to its registered marker format.
pub fn should_skip(excluded_format_present: bool) -> bool {
    excluded_format_present
}

/// Maximum time spent retrying a clipboard that is temporarily locked by another process.
pub const CLIPBOARD_RETRY_LIMIT: Duration = Duration::from_millis(500);

#[cfg(windows)]
mod win32 {
    use super::{CLIPBOARD_RETRY_LIMIT, decode_nul_terminated, should_skip};
    use std::io;
    use std::thread;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{GetLastError, HWND};
    use windows_sys::Win32::System::DataExchange::{
        AddClipboardFormatListener, CloseClipboard, GetClipboardData, GetClipboardSequenceNumber,
        IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
        RemoveClipboardFormatListener,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;

    const RETRY_INTERVAL: Duration = Duration::from_millis(10);

    /// Get the current sequence without reading clipboard contents.
    pub fn sequence_number() -> io::Result<u32> {
        // SAFETY: This query has no input and does not open clipboard contents.
        let sequence = unsafe { GetClipboardSequenceNumber() };
        if sequence == 0 {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "clipboard sequence unavailable",
            ))
        } else {
            Ok(sequence)
        }
    }

    /// Register a caller-owned window for WM_CLIPBOARDUPDATE notifications.
    pub fn add_listener(hwnd: HWND) -> io::Result<()> {
        // SAFETY: HWND is supplied by the caller and remains owned by that caller.
        if unsafe { AddClipboardFormatListener(hwnd) } != 0 {
            Ok(())
        } else {
            Err(last_error())
        }
    }

    /// Unregister a caller-owned window from WM_CLIPBOARDUPDATE notifications.
    pub fn remove_listener(hwnd: HWND) -> io::Result<()> {
        // SAFETY: HWND is supplied by the caller; removal does not take ownership of it.
        if unsafe { RemoveClipboardFormatListener(hwnd) } != 0 {
            Ok(())
        } else {
            Err(last_error())
        }
    }

    /// Read CF_UNICODETEXT, skipping monitor-excluded clipboard contents.
    /// Call from a worker thread, never the UI/message-pump thread.
    pub fn read_text() -> io::Result<Option<String>> {
        let started = Instant::now();
        loop {
            // SAFETY: Null HWND means the current process requests clipboard access.
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                let result = read_open_clipboard();
                // SAFETY: This call pairs with the successful OpenClipboard above.
                let closed = unsafe { CloseClipboard() };
                if closed == 0 && result.is_ok() {
                    return Err(last_error());
                }
                return result;
            }
            if started.elapsed() >= CLIPBOARD_RETRY_LIMIT {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "clipboard remained busy",
                ));
            }
            thread::sleep(RETRY_INTERVAL);
        }
    }

    fn read_open_clipboard() -> io::Result<Option<String>> {
        let marker: Vec<u16> = "ExcludeClipboardContentFromMonitorProcessing\0"
            .encode_utf16()
            .collect();
        // SAFETY: marker is NUL terminated and remains alive for this call.
        let marker_format = unsafe { RegisterClipboardFormatW(marker.as_ptr()) };
        if marker_format == 0 {
            return Err(last_error());
        }
        // SAFETY: Clipboard is currently open, as required by this API.
        if should_skip(unsafe { IsClipboardFormatAvailable(marker_format) } != 0) {
            return Ok(None);
        }
        // SAFETY: Clipboard is currently open; querying an unavailable format returns null.
        let handle = unsafe { GetClipboardData(CF_UNICODETEXT as u32) };
        if handle.is_null() {
            return Ok(None);
        }
        // SAFETY: Clipboard ownership prevents the handle from being released while open.
        let pointer = unsafe { GlobalLock(handle) };
        if pointer.is_null() {
            return Ok(None);
        }
        let guard = GlobalUnlockGuard(handle);
        // GlobalSize bounds the scan even if another app supplied malformed data.
        // SAFETY: GetClipboardData returned a global-memory handle held open and locked.
        let bytes = unsafe { GlobalSize(handle) };
        if bytes < 2 || bytes % 2 != 0 || bytes / 2 > isize::MAX as usize / 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid clipboard text size",
            ));
        }
        // SAFETY: GlobalLock returned an aligned pointer to this GlobalSize-sized block.
        let units = unsafe { std::slice::from_raw_parts(pointer.cast::<u16>(), bytes / 2) };
        let text = decode_nul_terminated(units)?;
        drop(guard);
        Ok(Some(text))
    }

    struct GlobalUnlockGuard(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for GlobalUnlockGuard {
        fn drop(&mut self) {
            // SAFETY: This handle was successfully locked by GlobalLock in read_open_clipboard.
            unsafe { GlobalUnlock(self.0) };
        }
    }

    fn last_error() -> io::Error {
        // SAFETY: GetLastError has no preconditions.
        io::Error::from_raw_os_error(unsafe { GetLastError() } as i32)
    }
}

/// Get the current clipboard change sequence without reading text.
#[cfg(windows)]
pub fn clipboard_sequence_number() -> std::io::Result<u32> {
    win32::sequence_number()
}

/// Clipboard APIs are only available on Windows.
#[cfg(not(windows))]
pub fn clipboard_sequence_number() -> std::io::Result<u32> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "clipboard sequence requires Windows",
    ))
}

/// Register a window for clipboard update messages on Windows.
#[cfg(windows)]
pub fn add_clipboard_listener(hwnd: windows_sys::Win32::Foundation::HWND) -> std::io::Result<()> {
    win32::add_listener(hwnd)
}

/// Remove a previously registered clipboard listener on Windows.
#[cfg(windows)]
pub fn remove_clipboard_listener(
    hwnd: windows_sys::Win32::Foundation::HWND,
) -> std::io::Result<()> {
    win32::remove_listener(hwnd)
}

/// Read text after receiving WM_CLIPBOARDUPDATE. Returns None for skipped/non-text data.
#[cfg(windows)]
pub fn read_clipboard_text() -> std::io::Result<Option<String>> {
    win32::read_text()
}

/// Clipboard APIs are only available on Windows.
#[cfg(not(windows))]
pub fn add_clipboard_listener(_hwnd: isize) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "clipboard listener requires Windows",
    ))
}

/// Clipboard APIs are only available on Windows.
#[cfg(not(windows))]
pub fn remove_clipboard_listener(_hwnd: isize) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "clipboard listener requires Windows",
    ))
}

/// Clipboard APIs are only available on Windows.
#[cfg(not(windows))]
pub fn read_clipboard_text() -> std::io::Result<Option<String>> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "clipboard capture requires Windows",
    ))
}

#[cfg(test)]
mod tests {
    use super::{decode_nul_terminated, decode_utf16, should_skip};

    #[test]
    fn decodes_valid_utf16_and_replaces_unpaired_surrogates() {
        assert_eq!(decode_utf16(&[0x3042, 0xd800, 0x3044]), "あ\u{fffd}い");
        assert_eq!(decode_utf16(&[0xd83d, 0xde00]), "😀");
    }

    #[test]
    fn bounded_clipboard_text_requires_terminator() {
        assert_eq!(decode_nul_terminated(&[0x3042, 0, 0x3044]).unwrap(), "あ");
        assert_eq!(
            decode_nul_terminated(&[0x3042]).unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn skip_decision_tracks_monitor_exclusion_format() {
        assert!(should_skip(true));
        assert!(!should_skip(false));
    }
}
