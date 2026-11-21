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
    use std::cell::Cell;
    use std::io;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::ptr::{null, null_mut};
    use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{GetLastError, HWND, WAIT_OBJECT_0};
    use windows_sys::Win32::System::DataExchange::{
        AddClipboardFormatListener, CloseClipboard, GetClipboardData, GetClipboardSequenceNumber,
        IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
        RemoveClipboardFormatListener,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
    use windows_sys::Win32::System::Threading::{CreateEventW, INFINITE, SetEvent};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DestroyWindow, DispatchMessageW, HWND_MESSAGE, MSG, MWMO_INPUTAVAILABLE,
        MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, QS_ALLINPUT, WM_CLIPBOARDUPDATE,
        WM_QUIT,
    };

    const RETRY_INTERVAL: Duration = Duration::from_millis(10);

    /// Clipboard update notifications from a dedicated Windows message thread.
    pub struct ClipboardListener {
        changes: Receiver<()>,
        last_sequence: Cell<u32>,
        stop: OwnedHandle,
        worker: Option<JoinHandle<()>>,
    }

    impl ClipboardListener {
        /// Start listening without reading the current clipboard contents.
        pub fn new() -> io::Result<Self> {
            // SAFETY: Null arguments request default security and an unnamed event.
            let stop = unsafe { CreateEventW(null(), 1, 0, null()) };
            if stop.is_null() {
                return Err(last_error());
            }
            // SAFETY: CreateEventW returned a newly owned, valid event handle.
            let stop = unsafe { OwnedHandle::from_raw_handle(stop) };
            let worker_stop = stop.try_clone()?;
            let (ready_tx, ready_rx) = mpsc::sync_channel(1);
            let (changes_tx, changes) = mpsc::sync_channel(1);
            let worker = thread::Builder::new()
                .name("clipboard-listener".into())
                .spawn(move || {
                    let startup =
                        ClipboardWindow::new().and_then(|window| Ok((window, sequence_number()?)));
                    match startup {
                        Ok((window, sequence)) => {
                            if ready_tx.send(Ok(sequence)).is_ok() {
                                window.pump(&worker_stop, &changes_tx);
                            }
                        }
                        Err(error) => {
                            let _ = ready_tx.send(Err(error));
                        }
                    }
                })?;
            let sequence = match ready_rx.recv().unwrap_or_else(|_| Err(listener_stopped())) {
                Ok(sequence) => sequence,
                Err(error) => {
                    let _ = worker.join();
                    return Err(error);
                }
            };
            Ok(Self {
                changes,
                last_sequence: Cell::new(sequence),
                stop,
                worker: Some(worker),
            })
        }

        /// Wait for a new sequence, or return `None` when the timeout expires.
        ///
        /// A burst can coalesce into the current clipboard sequence. Read it from the
        /// caller using `read_clipboard_change`, so a replaced sequence is skipped.
        pub fn wait_for_change(&self, timeout: Duration) -> io::Result<Option<u32>> {
            let started = Instant::now();
            loop {
                match self
                    .changes
                    .recv_timeout(timeout.saturating_sub(started.elapsed()))
                {
                    Ok(()) => {
                        let sequence = sequence_number()?;
                        if sequence != self.last_sequence.replace(sequence) {
                            return Ok(Some(sequence));
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => return Ok(None),
                    Err(RecvTimeoutError::Disconnected) => return Err(listener_stopped()),
                }
                if started.elapsed() >= timeout {
                    return Ok(None);
                }
            }
        }
    }

    impl Drop for ClipboardListener {
        fn drop(&mut self) {
            // SAFETY: The event is owned by self and remains valid during this call.
            if unsafe { SetEvent(self.stop.as_raw_handle()) } != 0
                && let Some(worker) = self.worker.take()
            {
                let _ = worker.join();
            }
        }
    }

    fn listener_stopped() -> io::Error {
        io::Error::new(io::ErrorKind::BrokenPipe, "clipboard listener stopped")
    }

    struct ClipboardWindow(HWND);

    impl ClipboardWindow {
        fn new() -> io::Result<Self> {
            // SAFETY: STATIC is a predefined class and HWND_MESSAGE requests an invisible window.
            let hwnd = unsafe {
                CreateWindowExW(
                    0,
                    windows_sys::w!("STATIC"),
                    null(),
                    0,
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    null_mut(),
                    null_mut(),
                    null(),
                )
            };
            if hwnd.is_null() {
                return Err(last_error());
            }
            let window = Self(hwnd);
            add_listener(hwnd)?;
            Ok(window)
        }

        fn pump(&self, stop: &OwnedHandle, changes: &SyncSender<()>) {
            let stop = stop.as_raw_handle();
            let mut message = MSG::default();
            loop {
                // SAFETY: The handle array has one live event and the wait uses this thread's queue.
                let result = unsafe {
                    MsgWaitForMultipleObjectsEx(
                        1,
                        &stop,
                        INFINITE,
                        QS_ALLINPUT,
                        MWMO_INPUTAVAILABLE,
                    )
                };
                if result != WAIT_OBJECT_0 + 1 {
                    return;
                }
                // SAFETY: The message storage is writable and null selects this thread's queue.
                if unsafe { PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) } == 0 {
                    continue;
                }
                match message.message {
                    WM_QUIT => return,
                    WM_CLIPBOARDUPDATE if message.hwnd == self.0 => {
                        // One pending notification is enough to sample the latest clipboard state.
                        let _ = changes.try_send(());
                    }
                    _ => {
                        // SAFETY: PeekMessageW populated message with a message from this queue.
                        unsafe { DispatchMessageW(&message) };
                    }
                }
            }
        }
    }

    impl Drop for ClipboardWindow {
        fn drop(&mut self) {
            let _ = remove_listener(self.0);
            // SAFETY: The window is created, pumped and destroyed on the same thread.
            unsafe { DestroyWindow(self.0) };
        }
    }

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
    pub fn read_text(expected_sequence: Option<u32>) -> io::Result<Option<String>> {
        let started = Instant::now();
        loop {
            // SAFETY: Null HWND means the current process requests clipboard access.
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                let result = match expected_sequence {
                    Some(expected) => sequence_number().and_then(|current| {
                        if current == expected {
                            read_open_clipboard()
                        } else {
                            Ok(None)
                        }
                    }),
                    None => read_open_clipboard(),
                };
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

#[cfg(windows)]
pub use win32::ClipboardListener;

/// Clipboard update notifications are only available on Windows.
#[cfg(not(windows))]
pub struct ClipboardListener {
    _private: (),
}

#[cfg(not(windows))]
impl ClipboardListener {
    /// Return an unsupported error because the listener needs Windows.
    pub fn new() -> std::io::Result<Self> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "clipboard listener requires Windows",
        ))
    }

    /// Return an unsupported error because the listener needs Windows.
    pub fn wait_for_change(&self, _timeout: Duration) -> std::io::Result<Option<u32>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "clipboard listener requires Windows",
        ))
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
    win32::read_text(None)
}

/// Read text for exactly the observed clipboard sequence, skipping stale or excluded content.
#[cfg(windows)]
pub fn read_clipboard_change(sequence: u32) -> std::io::Result<Option<String>> {
    win32::read_text(Some(sequence))
}

/// Clipboard APIs are only available on Windows.
#[cfg(not(windows))]
pub fn read_clipboard_change(_sequence: u32) -> std::io::Result<Option<String>> {
    read_clipboard_text()
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
