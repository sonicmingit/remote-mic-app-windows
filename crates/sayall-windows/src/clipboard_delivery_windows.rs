//! Public Win32 clipboard and window identity operations. Never reads target text.
use windows::Win32::{
    Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND},
    System::{
        DataExchange::{
            CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardOwner,
            GetClipboardSequenceNumber, OpenClipboard, SetClipboardData,
        },
        Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE},
    },
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, IsWindowEnabled, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
        },
        WindowsAndMessaging::*,
    },
};

const UNICODE_TEXT: u32 = 13;

#[derive(Clone, Copy)]
pub(super) struct Target {
    pub root: isize,
    pub focus: isize,
    root_pid: u32,
    root_tid: u32,
    focus_pid: u32,
    pub focus_tid: u32,
}
impl Target {
    pub unsafe fn capture() -> Option<Self> {
        let root = GetForegroundWindow();
        let mut root_pid = 0;
        let root_tid = GetWindowThreadProcessId(root, Some(&mut root_pid));
        if root_tid == 0 {
            return None;
        }
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        GetGUIThreadInfo(root_tid, &mut info).ok()?;
        let focus = info.hwndFocus;
        let mut focus_pid = 0;
        let focus_tid = GetWindowThreadProcessId(focus, Some(&mut focus_pid));
        if focus_tid == 0 || (focus != root && !IsChild(root, focus).as_bool()) {
            return None;
        }
        Some(Self {
            root: root.0 as isize,
            focus: focus.0 as isize,
            root_pid,
            root_tid,
            focus_pid,
            focus_tid,
        })
    }
    pub fn root_hwnd(self) -> HWND {
        HWND(self.root as *mut _)
    }
    pub fn focus_hwnd(self) -> HWND {
        HWND(self.focus as *mut _)
    }
    pub unsafe fn valid(self) -> bool {
        let root = self.root_hwnd();
        let focus = self.focus_hwnd();
        let mut root_pid = 0;
        let mut focus_pid = 0;
        IsWindow(Some(root)).as_bool()
            && IsWindow(Some(focus)).as_bool()
            && IsWindowVisible(root).as_bool()
            && IsWindowEnabled(root).as_bool()
            && IsWindowEnabled(focus).as_bool()
            && IsWindowVisible(focus).as_bool()
            && GetWindowThreadProcessId(root, Some(&mut root_pid)) == self.root_tid
            && root_pid == self.root_pid
            && GetWindowThreadProcessId(focus, Some(&mut focus_pid)) == self.focus_tid
            && focus_pid == self.focus_pid
            && (root == focus || IsChild(root, focus).as_bool())
    }
    pub unsafe fn focused(self) -> bool {
        if !self.valid() || GetForegroundWindow() != self.root_hwnd() {
            return false;
        }
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        GetGUIThreadInfo(self.focus_tid, &mut info).is_ok() && info.hwndFocus == self.focus_hwnd()
    }
}

pub(super) unsafe fn modifiers_held() -> bool {
    [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN]
        .iter()
        .any(|key| GetAsyncKeyState(key.0 as i32) < 0)
}

#[derive(Clone, Copy)]
pub(super) struct ClipboardStamp {
    sequence: u32,
    owner: isize,
}
impl ClipboardStamp {
    pub unsafe fn unchanged(self) -> bool {
        GetClipboardSequenceNumber() == self.sequence
            && GetClipboardOwner().is_ok_and(|owner| owner.0 as isize == self.owner)
    }
}
struct Opened;
impl Drop for Opened {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseClipboard();
        }
    }
}
struct Allocation(HGLOBAL);
impl Drop for Allocation {
    fn drop(&mut self) {
        if !self.0 .0.is_null() {
            unsafe {
                let _ = GlobalFree(Some(self.0));
            }
        }
    }
}

/// None = another application owns the clipboard lock; caller may retry without
/// hiding the receiver. Readback is strictly limited to our freshly written data.
pub(super) unsafe fn copy(owner: HWND, text: &str) -> Result<Option<ClipboardStamp>, &'static str> {
    let data: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let mut allocation =
        Allocation(GlobalAlloc(GMEM_MOVEABLE, data.len() * 2).map_err(|_| "allocate_failed")?);
    let pointer = GlobalLock(allocation.0);
    if pointer.is_null() {
        return Err("allocation_lock_failed");
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), pointer.cast::<u16>(), data.len());
    let _ = GlobalUnlock(allocation.0);
    if OpenClipboard(Some(owner)).is_err() {
        return Ok(None);
    }
    let opened = Opened;
    EmptyClipboard().map_err(|_| "empty_failed")?;
    SetClipboardData(UNICODE_TEXT, Some(HANDLE(allocation.0 .0))).map_err(|_| "set_failed")?;
    allocation.0 = HGLOBAL::default(); // system owns it after SetClipboardData
    let readback = HGLOBAL(GetClipboardData(UNICODE_TEXT).map_err(|_| "get_failed")?.0);
    if GlobalSize(readback) < data.len() * 2 {
        return Err("readback_size_failed");
    }
    let pointer = GlobalLock(readback);
    if pointer.is_null() {
        return Err("readback_lock_failed");
    }
    let matches = std::slice::from_raw_parts(pointer.cast::<u16>(), data.len()) == data;
    let _ = GlobalUnlock(readback);
    if !matches {
        return Err("readback_mismatch");
    }
    let sequence_before_close = GetClipboardSequenceNumber();
    drop(opened);
    // On the measured Windows host CloseClipboard advances the sequence while
    // retaining our owner and Unicode data. Baseline only the completed write.
    // This is not a delay or a reason to ignore later changes before Ctrl+V.
    let stamp = ClipboardStamp {
        sequence: GetClipboardSequenceNumber(),
        owner: owner.0 as isize,
    };
    crate::gatt_note(format!(
        "clipboard_probe action=clipboard_close phase=observed sequence_updated_on_close={}",
        sequence_before_close != stamp.sequence
    ));
    if !stamp.unchanged() {
        return Err("clipboard_changed");
    }
    Ok(Some(stamp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::{
        core::{w, PCWSTR},
        Win32::Foundation::{LPARAM, WPARAM},
    };

    // Explicit opt-in because this replaces clipboard content. No visible window,
    // no foreground change, no global key injection, no previous clipboard read.
    #[test]
    #[ignore = "writes a synthetic string to the real Windows clipboard"]
    fn native_unicode_clipboard_readback_and_hidden_edit_paste() {
        unsafe {
            let owner = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("EDIT"),
                w!(""),
                WS_CHILD | WINDOW_STYLE(ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32),
                0,
                0,
                480,
                100,
                Some(owner),
                None,
                None,
                None,
            )
            .unwrap();
            let text = "SayAll 浮窗投送测试🙂\r\n第二行";
            SetWindowTextW(edit, w!("fixture")).unwrap();
            assert_eq!(
                GetWindowTextLengthW(edit),
                7,
                "hidden edit must accept native text"
            );
            SetWindowTextW(edit, w!("")).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let stamp = loop {
                if let Some(stamp) = copy(owner, text).unwrap() {
                    break stamp;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "clipboard stayed busy"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            assert!(stamp.unchanged());
            assert!(
                windows::Win32::System::DataExchange::IsClipboardFormatAvailable(UNICODE_TEXT)
                    .is_ok(),
                "Unicode format must be available after close"
            );
            while OpenClipboard(Some(owner)).is_err() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "clipboard stayed busy during readback"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let opened = Opened;
            let data = HGLOBAL(GetClipboardData(UNICODE_TEXT).unwrap().0);
            let pointer = GlobalLock(data).cast::<u16>();
            assert!(!pointer.is_null());
            let expected: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
            assert_eq!(
                std::slice::from_raw_parts(pointer, expected.len()),
                expected,
                "Unicode text must survive clipboard close"
            );
            let _ = GlobalUnlock(data);
            drop(opened);
            assert!(
                stamp.unchanged(),
                "read-only open/close must not change the stamp"
            );
            // Clipboard viewers may briefly lock the clipboard after CloseClipboard.
            // Retry only while this synthetic hidden editor is still entirely empty.
            // Production Ctrl+V is never replayed on a timer.
            while GetWindowTextLengthW(edit) == 0 {
                assert!(stamp.unchanged());
                assert!(
                    std::time::Instant::now() < deadline,
                    "hidden editor did not paste"
                );
                SendMessageW(edit, WM_PASTE, Some(WPARAM(0)), Some(LPARAM(0)));
                if GetWindowTextLengthW(edit) == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            let mut buffer = vec![0u16; GetWindowTextLengthW(edit) as usize + 1];
            let len = GetWindowTextW(edit, &mut buffer) as usize;
            assert_eq!(String::from_utf16(&buffer[..len]).unwrap(), text);
            let second = loop {
                if let Some(stamp) = copy(owner, "new clipboard").unwrap() {
                    break stamp;
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            assert!(!stamp.unchanged());
            assert!(second.unchanged());
            SetWindowTextW(edit, PCWSTR(w!("").as_ptr())).unwrap();
            DestroyWindow(edit).unwrap();
            DestroyWindow(owner).unwrap();
        }
    }
}
