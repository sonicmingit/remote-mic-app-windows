//! Native EDIT receiver for an explicitly enabled experiment. BLE only posts
//! commands; it never waits for window creation, focus, IME, or recognition.
//! This version intentionally never sends Ctrl+V: first establish IME ground truth.
use super::ProbeSession;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Mutex, OnceLock,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{CreateFontIndirectW, DeleteObject, HFONT, LOGFONTW},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{AttachThreadInput, GetCurrentThreadId},
        },
        UI::{
            HiDpi::GetDpiForSystem,
            Input::KeyboardAndMouse::{GetAsyncKeyState, GetFocus, SetFocus, VK_RMENU},
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::*,
        },
    },
};

const COMMAND: u32 = WM_APP + 73;
const INPUT_CHANGED: u32 = WM_APP + 74;
const IME_START: u32 = WM_APP + 75;
const IME_END: u32 = WM_APP + 76;
const EDIT_ID: usize = 101;
static ENABLED: AtomicBool = AtomicBool::new(false);
static NEXT: AtomicU64 = AtomicU64::new(0);
static ACTIVE: AtomicU64 = AtomicU64::new(0);
static RUNTIME: OnceLock<Mutex<Option<Runtime>>> = OnceLock::new();

enum Command {
    Begin {
        generation: u64,
        origin: isize,
        requested: Instant,
    },
    Release(u64),
    Cancel(&'static str),
    Quit,
}
struct Runtime {
    sender: Sender<Command>,
    hwnd: isize,
    thread: JoinHandle<()>,
}
struct Context {
    receiver: Receiver<Command>,
    hwnd: HWND,
    edit: HWND,
    status: HWND,
    english: bool,
    state: ProbeSession,
    origin: isize,
    started: Instant,
    font: HFONT,
    scale: i32,
    attached_threads: Option<(u32, u32)>,
}
fn log(action: &str, generation: u64, fields: &str) {
    crate::gatt_note(format!(
        "clipboard_probe action={action} generation={generation} {fields}"
    ));
}
fn slot() -> &'static Mutex<Option<Runtime>> {
    RUNTIME.get_or_init(|| Mutex::new(None))
}
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

pub fn set_enabled(value: bool, english: bool) -> Result<bool, String> {
    if !value {
        shutdown();
        return Ok(false);
    }
    if enabled() {
        return Ok(true);
    }
    shutdown();
    let (sender, receiver) = mpsc::channel();
    let (ready, result) = mpsc::sync_channel(1);
    let worker = std::thread::Builder::new()
        .name("sayall-ime-probe".into())
        .spawn(move || unsafe { run(receiver, ready, english) })
        .map_err(|_| "probe_thread_unavailable")?;
    match result.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(hwnd)) => {
            *slot().lock().unwrap_or_else(|p| p.into_inner()) = Some(Runtime {
                sender,
                hwnd,
                thread: worker,
            });
            ENABLED.store(true, Ordering::Release);
            log("enable", 0, "phase=completed result=passed");
            Ok(true)
        }
        _ => {
            // A late window checks the closed readiness receiver and destroys itself.
            log(
                "enable",
                0,
                "phase=completed result=failed reason=window_unavailable",
            );
            Err("probe_window_unavailable".into())
        }
    }
}
fn post(command: Command) {
    let runtime = slot().lock().unwrap_or_else(|p| p.into_inner());
    if let Some(runtime) = runtime.as_ref() {
        let sent = runtime.sender.send(command).is_ok();
        let posted = unsafe {
            PostMessageW(
                Some(HWND(runtime.hwnd as *mut _)),
                COMMAND,
                WPARAM(0),
                LPARAM(0),
            )
        }
        .is_ok();
        if !sent || !posted {
            ENABLED.store(false, Ordering::Release);
            ACTIVE.store(0, Ordering::Release);
            log(
                "post",
                0,
                "phase=completed result=failed reason=worker_unavailable",
            );
        }
    }
}
pub fn begin() {
    if !enabled() {
        return;
    }
    let generation = NEXT.fetch_add(1, Ordering::AcqRel) + 1;
    ACTIVE.store(generation, Ordering::Release);
    let origin = unsafe { GetForegroundWindow() }.0 as isize;
    post(Command::Begin {
        generation,
        origin,
        requested: Instant::now(),
    });
}
pub fn release() {
    let generation = ACTIVE.load(Ordering::Acquire);
    if generation != 0 {
        post(Command::Release(generation));
    }
}
pub fn cancel(reason: &'static str) {
    if ACTIVE.swap(0, Ordering::AcqRel) != 0 {
        post(Command::Cancel(reason));
    }
}
pub fn shutdown() {
    ENABLED.store(false, Ordering::Release);
    ACTIVE.store(0, Ordering::Release);
    let runtime = slot().lock().unwrap_or_else(|p| p.into_inner()).take();
    if let Some(runtime) = runtime {
        let _ = runtime.sender.send(Command::Quit);
        let _ = unsafe {
            PostMessageW(
                Some(HWND(runtime.hwnd as *mut _)),
                COMMAND,
                WPARAM(0),
                LPARAM(0),
            )
        };
        let deadline = Instant::now() + Duration::from_millis(500);
        while !runtime.thread.is_finished() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if runtime.thread.is_finished() {
            let _ = runtime.thread.join();
        } else {
            log(
                "shutdown",
                0,
                "phase=completed result=unknown reason=window_thread_timeout",
            );
        }
    }
}
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
unsafe fn label(hwnd: HWND, value: &str) {
    let value = wide(value);
    let _ = SetWindowTextW(hwnd, PCWSTR(value.as_ptr()));
}

impl Context {
    unsafe fn detach_input(&mut self) -> bool {
        let Some((foreground, current)) = self.attached_threads else {
            return true;
        };
        let detached = AttachThreadInput(foreground, current, false).as_bool();
        log(
            "detach_input",
            self.state.generation,
            &format!(
                "phase=completed result={}",
                if detached { "passed" } else { "failed" }
            ),
        );
        if detached {
            self.attached_threads = None;
        }
        detached
    }
    unsafe fn focus_editor(&mut self) -> bool {
        // Same public attach/detach pattern used by app_launcher, without its Alt
        // fallback: the real voice shortcut may already be physically held here.
        if !self.detach_input() {
            return false;
        }
        let _ = SetForegroundWindow(self.hwnd);
        let _ = SetFocus(Some(self.edit));
        if GetForegroundWindow() != self.hwnd {
            let foreground = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let current = GetCurrentThreadId();
            if foreground != 0
                && foreground != current
                && AttachThreadInput(foreground, current, true).as_bool()
            {
                self.attached_threads = Some((foreground, current));
                let _ = SetForegroundWindow(self.hwnd);
                let _ = SetFocus(Some(self.edit));
                if !self.detach_input() {
                    return false;
                }
            }
        }
        GetForegroundWindow() == self.hwnd && GetFocus() == self.edit
    }
    unsafe fn create_editor(&mut self, generation: u64) -> windows::core::Result<()> {
        // Use a fresh native edit/document for each press: old IME callbacks belong
        // to the destroyed control. Posted notifications also carry its generation.
        if !self.edit.0.is_null() {
            let _ = DestroyWindow(self.edit);
            self.edit = HWND::default();
        }
        let scale = |n: i32| n * self.scale / 96;
        self.edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            w!(""),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WS_VSCROLL
                | WINDOW_STYLE(ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32),
            scale(12),
            scale(12),
            scale(480),
            scale(100),
            Some(self.hwnd),
            Some(HMENU(EDIT_ID as *mut _)),
            Some(GetModuleHandleW(None)?.into()),
            None,
        )?;
        SetWindowLongPtrW(self.edit, GWLP_USERDATA, generation as isize);
        if !SetWindowSubclass(self.edit, Some(edit_proc), 1, generation as usize).as_bool() {
            return Err(windows::core::Error::from_thread());
        }
        SendMessageW(
            self.edit,
            WM_SETFONT,
            Some(WPARAM(self.font.0 as usize)),
            Some(LPARAM(1)),
        );
        Ok(())
    }
    unsafe fn receive(&mut self) {
        while let Ok(command) = self.receiver.try_recv() {
            match command {
                Command::Begin {
                    generation,
                    origin,
                    requested,
                } => {
                    if ACTIVE.load(Ordering::Acquire) != generation || !enabled() {
                        continue;
                    }
                    if self.state.generation != 0 {
                        log(
                            "cancel",
                            self.state.generation,
                            "phase=completed reason=replaced",
                        );
                    }
                    self.origin = if origin == self.hwnd.0 as isize {
                        self.origin
                    } else {
                        origin
                    };
                    self.started = requested;
                    self.state.cancel();
                    if self.create_editor(generation).is_err() {
                        log(
                            "begin",
                            generation,
                            "phase=completed result=failed reason=editor_unavailable",
                        );
                        ACTIVE
                            .compare_exchange(generation, 0, Ordering::AcqRel, Ordering::Acquire)
                            .ok();
                        continue;
                    }
                    self.state.begin(generation);
                    label(
                        self.status,
                        if self.english {
                            "Listening through Doubao. Release the voice button when finished."
                        } else {
                            "正在接收豆包输入，请按住语音键说话。"
                        },
                    );
                    let key_down = GetAsyncKeyState(VK_RMENU.0 as i32) < 0;
                    let _ = ShowWindow(self.hwnd, SW_SHOWNORMAL);
                    let focused = self.focus_editor();
                    let foreground_matches = GetForegroundWindow() == self.hwnd;
                    let editor_matches = GetFocus() == self.edit;
                    log("focus", generation, &format!("phase=completed result={} foreground_matches={foreground_matches} editor_matches={editor_matches} key_already_down={key_down} elapsed_ms={} origin_present={}", if focused { "passed" } else { "failed" }, requested.elapsed().as_millis(), self.origin != 0));
                    if !focused {
                        label(
                            self.status,
                            if self.english {
                                "Focus was not acquired. Click the text box and try again."
                            } else {
                                "浮窗未获得焦点，请点输入框后再试。本轮焦点切换需要核验。"
                            },
                        );
                    }
                }
                Command::Release(generation) => {
                    if self.state.release(generation) {
                        log(
                            "release",
                            generation,
                            &format!(
                                "phase=completed chars={} composing={} elapsed_ms={}",
                                self.state.chars,
                                self.state.composing,
                                self.started.elapsed().as_millis()
                            ),
                        );
                        self.refresh();
                    }
                }
                Command::Cancel(reason) => {
                    log(
                        "cancel",
                        self.state.generation,
                        &format!("phase=completed reason={reason} chars={}", self.state.chars),
                    );
                    self.state.cancel();
                    let _ = ShowWindow(self.hwnd, SW_HIDE);
                }
                Command::Quit => {
                    self.state.cancel();
                    let _ = DestroyWindow(self.hwnd);
                }
            }
        }
    }
    unsafe fn refresh(&self) {
        if self.state.generation == 0 {
            return;
        }
        let value = if self.english {
            format!(
                "{} characters · {}. Capture experiment only; automatic paste is not enabled.",
                self.state.chars,
                if self.state.released {
                    "button released; waiting for / inspecting IME text"
                } else {
                    "voice button held"
                }
            )
        } else {
            format!(
                "已接收 {} 字 · {}。本轮仅验证接字，暂不自动粘贴。",
                self.state.chars,
                if self.state.released {
                    "已松开，等待并检查豆包结果"
                } else {
                    "正在按住说话"
                }
            )
        };
        label(self.status, &value);
    }
    unsafe fn changed(&mut self, composing: Option<bool>) {
        let generation = self.state.generation;
        if generation == 0 || ACTIVE.load(Ordering::Acquire) != generation {
            return;
        }
        let len = GetWindowTextLengthW(self.edit).max(0) as usize;
        if len > 200_000 {
            log(
                "input",
                generation,
                "phase=completed result=failed reason=text_too_large",
            );
            return;
        }
        let mut buffer = vec![0u16; len + 1];
        let read = GetWindowTextW(self.edit, &mut buffer).max(0) as usize;
        let chars = String::from_utf16_lossy(&buffer[..read]).chars().count();
        let composing = composing.unwrap_or(self.state.composing);
        self.state.input(generation, chars, composing);
        log(
            "input",
            generation,
            &format!(
                "phase=observed chars={chars} composing={composing} after_release={} elapsed_ms={}",
                self.state.released,
                self.started.elapsed().as_millis()
            ),
        );
        self.refresh();
    }
}

unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    message: u32,
    wp: WPARAM,
    lp: LPARAM,
    _id: usize,
    generation: usize,
) -> LRESULT {
    let notification = match message {
        WM_IME_STARTCOMPOSITION => Some(IME_START),
        WM_IME_ENDCOMPOSITION => Some(IME_END),
        _ => None,
    };
    if let Some(message) = notification {
        let _ = PostMessageW(GetParent(hwnd).ok(), message, WPARAM(generation), LPARAM(0));
    }
    if message == WM_NCDESTROY {
        let _ = RemoveWindowSubclass(hwnd, Some(edit_proc), 1);
    }
    DefSubclassProc(hwnd, message, wp, lp)
}
unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if message == WM_NCCREATE {
        let create = &*(lp.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
    }
    // Native controls can synchronously reenter this procedure. Those notifications
    // only POST messages: never create a second mutable Context borrow reentrantly.
    if message == WM_COMMAND
        && (wp.0 & 0xffff) == EDIT_ID
        && ((wp.0 >> 16) & 0xffff) == EN_CHANGE as usize
    {
        let source_generation = GetWindowLongPtrW(HWND(lp.0 as *mut _), GWLP_USERDATA) as usize;
        let _ = PostMessageW(
            Some(hwnd),
            INPUT_CHANGED,
            WPARAM(source_generation),
            LPARAM(0),
        );
        return LRESULT(0);
    }
    let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Context;
    if !pointer.is_null() {
        match message {
            COMMAND => {
                (*pointer).receive();
                return LRESULT(0);
            }
            INPUT_CHANGED => {
                if wp.0 as u64 == (*pointer).state.generation {
                    (*pointer).changed(None);
                }
                return LRESULT(0);
            }
            IME_START | IME_END => {
                if wp.0 as u64 != (*pointer).state.generation {
                    return LRESULT(0);
                }
                log(
                    if message == IME_START {
                        "ime_start"
                    } else {
                        "ime_end"
                    },
                    (*pointer).state.generation,
                    "phase=observed",
                );
                (*pointer).changed(Some(message == IME_START));
                return LRESULT(0);
            }
            WM_CLOSE => {
                ACTIVE.store(0, Ordering::Release);
                (*pointer).state.cancel();
                let _ = ShowWindow(hwnd, SW_HIDE);
                return LRESULT(0);
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                return LRESULT(0);
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            }
            _ => {}
        }
    }
    DefWindowProcW(hwnd, message, wp, lp)
}

unsafe fn run(
    receiver: Receiver<Command>,
    ready: mpsc::SyncSender<Result<isize, &'static str>>,
    english: bool,
) {
    let dpi = GetDpiForSystem().max(96) as i32;
    let mut logfont = LOGFONTW {
        lfHeight: -(12 * dpi / 72),
        ..Default::default()
    };
    let face = wide("Microsoft YaHei UI");
    logfont.lfFaceName[..face.len()].copy_from_slice(&face);
    let font = CreateFontIndirectW(&logfont);
    let mut context = Box::new(Context {
        receiver,
        hwnd: HWND::default(),
        edit: HWND::default(),
        status: HWND::default(),
        english,
        state: ProbeSession::default(),
        origin: 0,
        started: Instant::now(),
        font,
        scale: dpi,
        attached_threads: None,
    });
    let result = (|| -> windows::core::Result<HWND> {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: w!("SayAllClipboardCaptureProbe"),
            hCursor: LoadCursorW(None, IDC_IBEAM)?,
            ..Default::default()
        };
        RegisterClassW(&class);
        let title = wide(if english {
            "Doubao Capture Check"
        } else {
            "豆包浮窗接字验证"
        });
        let scale = |n: i32| n * dpi / 96;
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class.lpszClassName,
            PCWSTR(title.as_ptr()),
            WS_POPUP | WS_CAPTION | WS_SYSMENU,
            (GetSystemMetrics(SM_CXSCREEN) - scale(520)) / 2,
            GetSystemMetrics(SM_CYSCREEN) - scale(330),
            scale(520),
            scale(260),
            None,
            None,
            Some(instance.into()),
            Some((&mut *context as *mut Context).cast()),
        )?;
        context.hwnd = hwnd;
        context.create_editor(0)?;
        context.status = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!(""),
            WS_CHILD | WS_VISIBLE,
            scale(12),
            scale(122),
            scale(480),
            scale(100),
            Some(hwnd),
            None,
            Some(instance.into()),
            None,
        )?;
        SendMessageW(
            context.status,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        Ok(hwnd)
    })();
    match result {
        Ok(hwnd) => {
            if ready.send(Ok(hwnd.0 as isize)).is_ok() {
                let mut message = MSG::default();
                while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
        Err(_) => {
            let _ = ready.send(Err("window_unavailable"));
        }
    }
    context.detach_input();
    if !context.hwnd.0.is_null() && IsWindow(Some(context.hwnd)).as_bool() {
        let _ = DestroyWindow(context.hwnd);
    }
    if !font.0.is_null() {
        let _ = DeleteObject(font.into());
    }
    log("worker_exit", 0, "phase=completed result=passed");
}
