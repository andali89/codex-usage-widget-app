//! Small Win32 boundary: own-window styles/messages and event-driven pointer regions.
//! Mouse input is always forwarded. No pointer history or other-window text is retained.
use crate::{
    geometry::{corridor_contains, Rect},
    interaction::Surface,
    service::{Action, Service},
    windows::Input,
};
use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};
use tauri::{Manager, WebviewWindow};
use windows_sys::Win32::{
    Foundation::*,
    UI::{Accessibility::*, Shell::*, WindowsAndMessaging::*},
};

pub fn app_identity(identifier: &str) -> Result<(), &'static str> {
    let wide = identifier.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    if identifier.contains('\0') || wide.len() > 129 {
        return Err("native-app-identity-invalid");
    }
    if unsafe { SetCurrentProcessExplicitAppUserModelID(wide.as_ptr()) } < 0 {
        Err("native-app-identity-unavailable")
    } else {
        Ok(())
    }
}

pub fn double_click_interval() -> u64 {
    unsafe { windows_sys::Win32::UI::Input::KeyboardAndMouse::GetDoubleClickTime() as u64 }
}

pub fn identity_matches(identifier: &str) -> bool {
    let mut value = std::ptr::null_mut();
    if unsafe { GetCurrentProcessExplicitAppUserModelID(&mut value) } < 0 || value.is_null() {
        return false;
    }
    let mut actual = Vec::new();
    for index in 0..=128 {
        let ch = unsafe { *value.add(index) };
        if ch == 0 {
            break;
        }
        actual.push(ch);
    }
    unsafe {
        windows_sys::Win32::System::Com::CoTaskMemFree(value.cast());
    }
    actual == identifier.encode_utf16().collect::<Vec<_>>()
}

#[derive(Clone, Copy, Default)]
pub struct Regions {
    pub generation: u64,
    pub trigger: Option<Rect>,
    pub preview: Option<Rect>,
    pub popup: Option<Rect>,
    pub monitor: Option<Rect>,
    pub second_monitor: Option<Rect>,
    pub shell: Option<Rect>,
    pub tray: Option<Rect>,
    pub source: Surface,
}
struct Hooks {
    service: Service,
    regions: Regions,
    last_pointer: Option<(u64, Surface, bool, bool, bool)>,
    last_menu: bool,
    mouse: usize,
    foreground: usize,
    location: usize,
    fullscreen: Option<(bool, bool, bool)>,
    control: usize,
    session_registered: bool,
}
static HOOKS: OnceLock<Mutex<Hooks>> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowPolicy {
    pub tool: bool,
    pub no_activate: bool,
    pub topmost: bool,
    pub shadow: bool,
    pub standard: Option<bool>,
}
#[derive(Clone, Copy)]
struct AppliedPolicy {
    policy: WindowPolicy,
    applying: bool,
    repair_queued: bool,
}
static POLICIES: OnceLock<Mutex<HashMap<usize, AppliedPolicy>>> = OnceLock::new();

fn policy_mismatch(policy: WindowPolicy, flags: u32) -> bool {
    (flags & WS_EX_TOPMOST != 0) != policy.topmost
        || (flags & WS_EX_NOACTIVATE != 0) != policy.no_activate
        || (flags & WS_EX_TOOLWINDOW != 0) != policy.tool
}

// Tauri setters execute synchronously when called on the owning UI thread. Mixing
// their queued style rewrites with service-thread SetWindowLong snapshots loses
// native flags. Keep both caches coherent, then verify the HWND in the same task.
fn on_window_thread<T: Send + 'static>(
    window: &WebviewWindow,
    operation: impl FnOnce(WebviewWindow) -> Result<T, &'static str> + Send + 'static,
) -> Result<T, &'static str> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    let copy = window.clone();
    window
        .run_on_main_thread(move || {
            let _ = send.send(operation(copy));
        })
        .map_err(|_| "native-window-thread")?;
    receive.recv().map_err(|_| "native-window-completion")?
}

pub fn configure(window: &WebviewWindow, policy: WindowPolicy) -> Result<(), &'static str> {
    on_window_thread(window, move |window| {
        let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as usize;
        let previous = {
            let mut policies = POLICIES
                .get_or_init(Default::default)
                .lock()
                .map_err(|_| "native-window-policy")?;
            policies.insert(
                hwnd,
                AppliedPolicy {
                    policy,
                    applying: true,
                    repair_queued: false,
                },
            )
        };
        let result = (|| {
            if previous.is_none_or(|p| p.policy.topmost != policy.topmost) {
                window
                    .set_always_on_top(policy.topmost)
                    .map_err(|_| "native-topmost")?;
            }
            if previous.is_none_or(|p| p.policy.no_activate != policy.no_activate) {
                window
                    .set_focusable(!policy.no_activate)
                    .map_err(|_| "native-focusable")?;
            }
            if previous.is_none_or(|p| p.policy.shadow != policy.shadow) {
                window
                    .set_shadow(policy.shadow)
                    .map_err(|_| "native-shadow")?;
            }
            if let Some(standard) = policy.standard {
                if previous.is_none_or(|p| p.policy.standard != policy.standard) {
                    window
                        .set_decorations(standard)
                        .map_err(|_| "native-decorations")?;
                    window
                        .set_resizable(standard)
                        .map_err(|_| "native-resizable")?;
                    window
                        .set_skip_taskbar(!standard)
                        .map_err(|_| "native-taskbar")?;
                    window
                        .set_min_size(Some(tauri::LogicalSize::new(
                            if standard { 380.0 } else { 280.0 },
                            if standard { 420.0 } else { 300.0 },
                        )))
                        .map_err(|_| "native-minimum-size")?;
                }
            }
            style(&window, policy.tool, policy.no_activate)?;
            topmost(&window, policy.topmost)?;
            let flags = unsafe { GetWindowLongPtrW(hwnd as HWND, GWL_EXSTYLE) as u32 };
            if policy_mismatch(policy, flags) {
                Err("native-window-policy-mismatch")
            } else {
                Ok(())
            }
        })();
        if let Ok(mut policies) = POLICIES.get_or_init(Default::default).lock() {
            if result.is_ok() {
                if let Some(entry) = policies.get_mut(&hwnd) {
                    entry.applying = false;
                }
            } else {
                policies.remove(&hwnd);
            }
        }
        result
    })
}

static CONTROL_RECONCILES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn control_reconciles() -> u64 {
    CONTROL_RECONCILES.load(std::sync::atomic::Ordering::SeqCst)
}
pub fn post_control_check() -> Result<u64, &'static str> {
    let hooks = HOOKS
        .get()
        .and_then(|h| h.lock().ok())
        .ok_or("native-control-unavailable")?;
    let hwnd = hooks.control as HWND;
    if !hooks.session_registered
        || unsafe { IsWindow(hwnd) } == 0
        || unsafe { IsWindowVisible(hwnd) } != 0
    {
        return Err("native-control-lifetime");
    }
    let before = control_reconciles();
    if unsafe { PostMessageW(hwnd, WM_TIMECHANGE, 0, 0) } == 0 {
        return Err("native-control-post");
    }
    Ok(before)
}

fn observe(x: f64, y: f64, click: bool) {
    let Some(mut hooks) = HOOKS.get().and_then(|h| h.try_lock().ok()) else {
        return;
    };
    let r = hooks.regions;
    let tray = r.tray.is_some_and(|r| r.contains(x, y));
    let shell = r.shell.is_some_and(|r| r.contains(x, y));
    let source = if tray {
        Surface::Tray
    } else if shell {
        Surface::Shell
    } else {
        r.source
    };
    let trigger = tray || shell;
    let preview = r.preview.is_some_and(|r| r.contains(x, y));
    let corridor = r
        .trigger
        .zip(r.preview)
        .is_some_and(|(a, b)| corridor_contains(a, b, x, y));
    let packet = (r.generation, source, trigger, preview, corridor);
    if hooks.last_pointer != Some(packet)
        && hooks
            .service
            .0
            .try_send(Action::Window(Input::Pointer {
                generation: r.generation,
                source,
                trigger,
                preview,
                corridor,
            }))
            .is_ok()
    {
        hooks.last_pointer = Some(packet);
    }
    let mut info: GUITHREADINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
    if unsafe { GetGUIThreadInfo(0, &mut info) } != 0 {
        let menu = info.flags & (GUI_INMENUMODE | GUI_POPUPMENUMODE | GUI_SYSTEMMENUMODE) != 0;
        if menu != hooks.last_menu
            && hooks
                .service
                .0
                .try_send(Action::Window(Input::Menu(menu)))
                .is_ok()
        {
            hooks.last_menu = menu;
        }
    }
    if click && r.popup.is_some_and(|r| !r.contains(x, y)) && !trigger && !preview {
        hooks.service.dispatch(Action::Window(Input::Outside));
    }
}

unsafe extern "system" fn mouse_callback(code: i32, message: WPARAM, data: LPARAM) -> LRESULT {
    if code >= 0
        && data != 0
        && [
            WM_MOUSEMOVE as usize,
            WM_LBUTTONDOWN as usize,
            WM_RBUTTONDOWN as usize,
        ]
        .contains(&message)
    {
        let packet = &*(data as *const MSLLHOOKSTRUCT);
        observe(
            packet.pt.x as f64,
            packet.pt.y as f64,
            message != WM_MOUSEMOVE as usize,
        );
    }
    CallNextHookEx(std::ptr::null_mut(), code, message, data)
}
unsafe extern "system" fn foreground_callback(
    _: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if let Some(mut hooks) = HOOKS.get().and_then(|h| h.try_lock().ok()) {
            hooks.fullscreen = None;
            // A competing topmost window can cover us while our style bit stays
            // set. Only external activation requests a one-shot stacking repair;
            // our own popup activation must not raise its anchor over the popup.
            hooks.service.dispatch_reliable(Action::Window(
                if pid != 0 && pid != std::process::id() {
                    Input::ExternalForeground
                } else {
                    Input::Environment
                },
            ));
        }
        return;
    }
    if hwnd != GetForegroundWindow() {
        return;
    }
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == std::process::id() {
        return;
    }
    let Some(mut hooks) = HOOKS.get().and_then(|h| h.try_lock().ok()) else {
        return;
    };
    let Some(monitor) = hooks.regions.monitor else {
        return;
    };
    let mut rect = RECT::default();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return;
    }
    let rect = Rect {
        x: rect.left as f64,
        y: rect.top as f64,
        width: (rect.right - rect.left) as f64,
        height: (rect.bottom - rect.top) as f64,
    };
    let full = (
        rect.covers_monitor(monitor),
        rect.covers_monitor(hooks.regions.second_monitor.unwrap_or(monitor)),
        monitor_bounds(hwnd).is_none_or(|monitor| rect.covers_monitor(monitor)),
    );
    if hooks.fullscreen != Some(full)
        && hooks
            .service
            .0
            .try_send(Action::Window(Input::Environment))
            .is_ok()
    {
        hooks.fullscreen = Some(full);
    }
}

struct WindowData {
    service: Service,
    surface: Surface,
}
unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    let context = &*(data as *const WindowData);
    let event = match message {
        WM_ENTERSIZEMOVE if context.surface == Surface::Main => Some(Input::MainUserSizing),
        WM_SYSCOMMAND
            if context.surface == Surface::Main
                && matches!(wparam as u32 & 0xfff0, SC_MAXIMIZE | SC_SIZE) =>
        {
            Some(Input::MainUserSizing)
        }
        WM_ENTERSIZEMOVE if context.surface == Surface::Shell => Some(Input::DragStarted),
        WM_EXITSIZEMOVE if context.surface == Surface::Shell => {
            // Capture at release, before the actor queue or pointer can advance.
            let mut rect = RECT::default();
            let mut point = POINT::default();
            let bounds = (GetWindowRect(hwnd, &mut rect) != 0).then_some(Rect {
                x: rect.left as f64,
                y: rect.top as f64,
                width: (rect.right - rect.left) as f64,
                height: (rect.bottom - rect.top) as f64,
            });
            let pointer =
                (GetCursorPos(&mut point) != 0).then_some((point.x as f64, point.y as f64));
            Some(Input::DragFinished {
                window: hwnd as usize,
                bounds,
                pointer,
            })
        }
        WM_DPICHANGED => Some(Input::Layout),
        WM_SIZE if context.surface == Surface::Main => {
            Some(Input::Minimized(wparam == SIZE_MINIMIZED as usize))
        }
        WM_EXITMENULOOP => Some(Input::Menu(false)),
        _ => None,
    };
    if let Some(event) = event {
        context.service.dispatch_reliable(Action::Window(event));
    }
    if matches!(message, WM_STYLECHANGED | WM_WINDOWPOSCHANGED) {
        let repair = POLICIES
            .get()
            .and_then(|policies| policies.lock().ok())
            .is_some_and(|mut policies| {
                let Some(entry) = policies.get_mut(&(hwnd as usize)) else {
                    return false;
                };
                if entry.applying
                    || entry.repair_queued
                    || !policy_mismatch(entry.policy, GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32)
                {
                    return false;
                }
                entry.repair_queued = true;
                true
            });
        if repair {
            context
                .service
                .dispatch_reliable(Action::Window(Input::Environment));
        }
    }
    if message == WM_NCDESTROY {
        if let Some(mut policies) = POLICIES.get().and_then(|p| p.lock().ok()) {
            policies.remove(&(hwnd as usize));
        }
        RemoveWindowSubclass(hwnd, Some(subclass), id);
        drop(Box::from_raw(data as *mut WindowData));
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

const CONTROL_CLASS: &str = "CodexUsageWidget.Coordinator";
unsafe extern "system" fn control_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let event = match message {
        WM_SETTINGCHANGE | WM_THEMECHANGED => {
            if let Some(hooks) = HOOKS.get().and_then(|h| h.lock().ok()) {
                hooks.service.dispatch(Action::Appearance);
            }
            Some(Input::Environment)
        }
        WM_DISPLAYCHANGE => Some(Input::Layout),
        WM_TIMECHANGE => {
            CONTROL_RECONCILES.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Some(Input::Reconcile)
        }
        WM_POWERBROADCAST if wparam == 0x12 || wparam == 7 => Some(Input::Reconcile),
        WM_POWERBROADCAST => Some(Input::Environment),
        WM_WTSSESSION_CHANGE if wparam == 7 => Some(Input::Lock(true)),
        WM_WTSSESSION_CHANGE if wparam == 8 => Some(Input::Lock(false)),
        _ => None,
    };
    if let Some(event) = event {
        if let Some(hooks) = HOOKS.get().and_then(|h| h.lock().ok()) {
            hooks.service.dispatch_reliable(Action::Window(event));
        }
    }
    DefWindowProcW(hwnd, message, wparam, lparam)
}
pub fn initialize(service: Service) -> Result<(), &'static str> {
    let _ = HOOKS.set(Mutex::new(Hooks {
        service: service.clone(),
        regions: Regions::default(),
        last_pointer: None,
        last_menu: false,
        mouse: 0,
        foreground: 0,
        location: 0,
        fullscreen: None,
        control: 0,
        session_registered: false,
    }));
    let class = CONTROL_CLASS
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let instance =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) };
    let description = WNDCLASSW {
        lpfnWndProc: Some(control_proc),
        hInstance: instance,
        lpszClassName: class.as_ptr(),
        ..Default::default()
    };
    if unsafe { RegisterClassW(&description) } == 0 {
        return Err("native-control-class-unavailable");
    }
    // A hidden top-level window receives broadcasts; a message-only HWND does not.
    // It has no WebView2, user controls, taskbar presence, activation or display mode.
    let control = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            class.as_ptr(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            instance,
            std::ptr::null(),
        )
    };
    if control.is_null() {
        unsafe {
            UnregisterClassW(class.as_ptr(), instance);
        };
        return Err("native-control-window-unavailable");
    }
    let session_registered = unsafe {
        windows_sys::Win32::System::RemoteDesktop::WTSRegisterSessionNotification(control, 0)
    } != 0;
    let mouse =
        unsafe { SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_callback), std::ptr::null_mut(), 0) };
    let foreground = unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            std::ptr::null_mut(),
            Some(foreground_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    let location = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_LOCATIONCHANGE,
            EVENT_OBJECT_LOCATIONCHANGE,
            std::ptr::null_mut(),
            Some(foreground_callback),
            0,
            0,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    if let Some(mut hooks) = HOOKS.get().and_then(|h| h.lock().ok()) {
        hooks.mouse = mouse as usize;
        hooks.foreground = foreground as usize;
        hooks.location = location as usize;
        hooks.control = control as usize;
        hooks.session_registered = session_registered;
    }
    if mouse.is_null() || foreground.is_null() || location.is_null() {
        Err("native-pointer-adapter-unavailable")
    } else if !session_registered {
        Err("native-session-events-unavailable")
    } else {
        Ok(())
    }
}

// Must be called on the window's owning Tauri thread (SetWindowSubclass constraint).
pub fn install_subclass(
    window: &WebviewWindow,
    service: Service,
    surface: Surface,
) -> Result<(), &'static str> {
    let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as HWND;
    let context = Box::into_raw(Box::new(WindowData { service, surface }));
    if unsafe { SetWindowSubclass(hwnd, Some(subclass), 0x435557, context as usize) } == 0 {
        unsafe {
            drop(Box::from_raw(context));
        }
        return Err("native-window-events-unavailable");
    }
    Ok(())
}

pub fn regions(regions: Regions) {
    if let Some(mut hooks) = HOOKS.get().and_then(|h| h.lock().ok()) {
        hooks.regions = regions;
    }
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) } != 0 {
        observe(point.x as f64, point.y as f64, false);
    }
}

pub fn style(window: &WebviewWindow, tool: bool, no_activate: bool) -> Result<(), &'static str> {
    let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as HWND;
    unsafe {
        let old = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let mut flags = old & !(WS_EX_TOOLWINDOW | WS_EX_APPWINDOW | WS_EX_NOACTIVATE);
        flags |= if tool {
            WS_EX_TOOLWINDOW
        } else {
            WS_EX_APPWINDOW
        };
        if no_activate {
            flags |= WS_EX_NOACTIVATE;
        }
        if flags == old {
            return Ok(());
        }
        SetLastError(0);
        let prior = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, flags as isize);
        if prior == 0 && GetLastError() != 0 {
            return Err("native-window-style");
        }
        if SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        ) == 0
        {
            return Err("native-window-frame");
        }
    }
    Ok(())
}

pub fn show_without_activation(window: &WebviewWindow) -> Result<(), &'static str> {
    let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as HWND;
    if unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    } == 0
    {
        Err("native-show-preview")
    } else {
        Ok(())
    }
}

pub fn raise_without_activation(window: &WebviewWindow) -> Result<(), &'static str> {
    on_window_thread(window, |window| {
        let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as HWND;
        if unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        } == 0
        {
            Err("native-raise-window")
        } else {
            Ok(())
        }
    })
}

pub fn topmost(window: &WebviewWindow, enabled: bool) -> Result<(), &'static str> {
    let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as HWND;
    unsafe {
        let actual = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0;
        if actual == enabled {
            return Ok(());
        }
        // Check the real HWND, not Tao's cached flags. Native style/frame changes
        // can diverge from that cache, and the asynchronous setter then does nothing.
        if SetWindowPos(
            hwnd,
            if enabled {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            },
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        ) == 0
        {
            return Err("native-topmost");
        }
        if (GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST != 0) != enabled {
            return Err("native-topmost-mismatch");
        }
    }
    Ok(())
}

pub fn is_above(window: &WebviewWindow, anchor: &WebviewWindow) -> bool {
    let (Ok(window), Ok(anchor)) = (window.hwnd(), anchor.hwnd()) else {
        return false;
    };
    let mut previous = unsafe { GetWindow(anchor.0 as HWND, GW_HWNDPREV) };
    while !previous.is_null() {
        if std::ptr::eq(previous, window.0) {
            return true;
        }
        previous = unsafe { GetWindow(previous, GW_HWNDPREV) };
    }
    false
}

pub fn above(window: &WebviewWindow, anchor: &WebviewWindow) -> Result<(), &'static str> {
    if !is_above(window, anchor) {
        raise_without_activation(window)?;
    }
    Ok(())
}

// Native clipping removes both paint and hit testing outside the rounded card,
// including on Windows 10 and when OS transparency is disabled.
pub fn round_corners(window: &WebviewWindow, radius: f64) -> Result<(), &'static str> {
    use windows_sys::Win32::Graphics::Gdi::*;
    let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as HWND;
    let size = window.outer_size().map_err(|_| "native-outer-size")?;
    let scale = window.scale_factor().map_err(|_| "native-window-scale")?;
    let diameter = (2.0 * radius * scale)
        .round()
        .min(size.width.min(size.height) as f64) as i32;
    unsafe {
        let region = CreateRoundRectRgn(
            0,
            0,
            size.width as i32 + 1,
            size.height as i32 + 1,
            diameter,
            diameter,
        );
        if region.is_null() {
            return Err("native-rounded-region");
        }
        let old = CreateRectRgn(0, 0, 0, 0);
        let unchanged =
            !old.is_null() && GetWindowRgn(hwnd, old) != 0 && EqualRgn(old, region) != 0;
        if !old.is_null() {
            DeleteObject(old);
        }
        if unchanged {
            DeleteObject(region);
        } else if SetWindowRgn(hwnd, region, 1) == 0 {
            // Windows owns the region only after a successful SetWindowRgn.
            DeleteObject(region);
            return Err("native-rounded-region");
        }
    }
    Ok(())
}

/// Query only the shell's appbar metadata; never inspect desktop artwork.
pub fn taskbar_safety(monitor: Rect) -> ([bool; 4], bool) {
    use windows_sys::Win32::UI::Shell::{
        SHAppBarMessage, ABM_GETAUTOHIDEBAREX, ABM_GETSTATE, ABS_AUTOHIDE, APPBARDATA,
    };
    let mut data = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        ..Default::default()
    };
    let tray_class = "Shell_TrayWnd\0".encode_utf16().collect::<Vec<_>>();
    if unsafe { FindWindowW(tray_class.as_ptr(), std::ptr::null()) }.is_null() {
        return ([true; 4], true);
    }
    if unsafe { SHAppBarMessage(ABM_GETSTATE, &mut data) } & ABS_AUTOHIDE as usize == 0 {
        return ([false; 4], false);
    }
    data.rc = windows_sys::Win32::Foundation::RECT {
        left: monitor.x as i32,
        top: monitor.y as i32,
        right: monitor.right() as i32,
        bottom: monitor.bottom() as i32,
    };
    let mut sides = [false; 4];
    for (edge, side) in sides.iter_mut().enumerate() {
        data.uEdge = edge as u32;
        *side = unsafe { SHAppBarMessage(ABM_GETAUTOHIDEBAREX, &mut data) } != 0;
    }
    if sides.iter().any(|s| *s) {
        (sides, false)
    } else {
        ([true; 4], true)
    }
}

pub fn composition_available() -> bool {
    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmIsCompositionEnabled(enabled: *mut i32) -> i32;
    }
    let mut enabled = 0;
    unsafe { DwmIsCompositionEnabled(&mut enabled) >= 0 && enabled != 0 }
}

pub fn has_rounded_corners(window: &WebviewWindow) -> bool {
    use windows_sys::Win32::Graphics::Gdi::*;
    let (Ok(hwnd), Ok(size)) = (window.hwnd(), window.outer_size()) else {
        return false;
    };
    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);
        if region.is_null() {
            return false;
        }
        let rounded = GetWindowRgn(hwnd.0 as HWND, region) != 0
            && PtInRegion(region, 0, 0) == 0
            && PtInRegion(region, size.width as i32 / 2, size.height as i32 / 2) != 0;
        DeleteObject(region);
        rounded
    }
}

pub fn hide(window: &WebviewWindow) {
    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            ShowWindow(hwnd.0 as HWND, SW_HIDE);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivationOutcome {
    Activated,
    Denied,
}

pub fn show_activated(window: &WebviewWindow) -> Result<ActivationOutcome, &'static str> {
    on_window_thread(window, |window| {
        let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as HWND;
        unsafe {
            ShowWindow(
                hwnd,
                if IsIconic(hwnd) != 0 {
                    SW_RESTORE
                } else {
                    SW_SHOWNOACTIVATE
                },
            );
        }
        if unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE,
            )
        } == 0
        {
            return Err("native-raise-window");
        }
        // No queued set_focus fallback: Tao uses cached visibility and can synthesize
        // an Alt key on denial. A single native request respects foreground rules.
        let accepted = unsafe { GetForegroundWindow() == hwnd || SetForegroundWindow(hwnd) != 0 };
        Ok(if accepted {
            ActivationOutcome::Activated
        } else {
            ActivationOutcome::Denied
        })
    })
}

pub fn request_attention(window: &WebviewWindow) -> Result<(), &'static str> {
    on_window_thread(window, |window| {
        let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as HWND;
        let info = FLASHWINFO {
            cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
            hwnd,
            dwFlags: FLASHW_TRAY,
            uCount: 3,
            dwTimeout: 0,
        };
        unsafe {
            FlashWindowEx(&info);
        }
        Ok(())
    })
}

pub fn content_outer_size(
    window: &WebviewWindow,
    content: (f64, f64),
    scale: f64,
) -> Result<(f64, f64), &'static str> {
    let outer = window.outer_size().map_err(|_| "native-outer-size")?;
    let client = window.inner_size().map_err(|_| "native-client-size")?;
    Ok(crate::geometry::content_outer_size(
        content,
        scale,
        (
            outer.width.saturating_sub(client.width),
            outer.height.saturating_sub(client.height),
        ),
    ))
}

pub fn place(window: &WebviewWindow, rect: Rect) -> Result<(), &'static str> {
    let hwnd = window.hwnd().map_err(|_| "native-window-handle")?.0 as HWND;
    let mut current = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut current) } == 0 {
        return Err("native-window-bounds");
    }
    let (x, y, width, height) = (
        rect.x.round() as i32,
        rect.y.round() as i32,
        rect.width.round() as i32,
        rect.height.round() as i32,
    );
    if (
        current.left,
        current.top,
        current.right - current.left,
        current.bottom - current.top,
    ) == (x, y, width, height)
    {
        return Ok(());
    }
    // Already-clamped outer bounds: content-sized callers include shadow insets;
    // decorated dashboards keep their existing outer-size policy.
    if unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    } == 0
    {
        Err("native-position")
    } else {
        Ok(())
    }
}

pub fn foreground_rect() -> Result<Option<Rect>, &'static str> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return Err("fullscreen-state-unavailable");
        }
        let mut process_id = 0;
        GetWindowThreadProcessId(hwnd, &mut process_id);
        if process_id == std::process::id()
            || hwnd == GetDesktopWindow()
            || hwnd == GetShellWindow()
            || IsIconic(hwnd) != 0
        {
            return Ok(None);
        }
        let mut class = [0u16; 64];
        let count = GetClassNameW(hwnd, class.as_mut_ptr(), class.len() as i32);
        let class = String::from_utf16_lossy(&class[..count.max(0) as usize]);
        if [
            "Progman",
            "WorkerW",
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
        ]
        .contains(&class.as_str())
        {
            return Ok(None);
        }
        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return Err("fullscreen-state-unavailable");
        }
        Ok(Some(Rect {
            x: rect.left as f64,
            y: rect.top as f64,
            width: (rect.right - rect.left) as f64,
            height: (rect.bottom - rect.top) as f64,
        }))
    }
}
fn monitor_bounds(hwnd: HWND) -> Option<Rect> {
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info: MONITORINFO = unsafe { std::mem::zeroed() };
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if monitor.is_null() || unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }
    let r = info.rcMonitor;
    Some(Rect {
        x: r.left as f64,
        y: r.top as f64,
        width: (r.right - r.left) as f64,
        height: (r.bottom - r.top) as f64,
    })
}
pub fn foreground_fullscreen() -> bool {
    match foreground_rect() {
        Err(_) => true,
        Ok(None) => false,
        Ok(Some(rect)) => monitor_bounds(unsafe { GetForegroundWindow() })
            .is_none_or(|monitor| rect.covers_monitor(monitor)),
    }
}

pub fn battery_saver() -> bool {
    let mut power: windows_sys::Win32::System::Power::SYSTEM_POWER_STATUS =
        unsafe { std::mem::zeroed() };
    unsafe {
        windows_sys::Win32::System::Power::GetSystemPowerStatus(&mut power) != 0
            && power.SystemStatusFlag == 1
    }
}
pub fn owned_focus(app: &tauri::AppHandle) -> bool {
    let foreground = unsafe { GetForegroundWindow() };
    app.webview_windows()
        .values()
        .any(|w| w.hwnd().is_ok_and(|h| std::ptr::eq(h.0, foreground)))
}
pub fn shutdown() {
    if let Some(mut hooks) = HOOKS.get().and_then(|h| h.lock().ok()) {
        unsafe {
            if hooks.mouse != 0 {
                UnhookWindowsHookEx(hooks.mouse as HHOOK);
            }
            if hooks.foreground != 0 {
                UnhookWinEvent(hooks.foreground as HWINEVENTHOOK);
            }
            if hooks.location != 0 {
                UnhookWinEvent(hooks.location as HWINEVENTHOOK);
            }
            if hooks.control != 0 {
                if hooks.session_registered {
                    windows_sys::Win32::System::RemoteDesktop::WTSUnRegisterSessionNotification(
                        hooks.control as HWND,
                    );
                }
                DestroyWindow(hooks.control as HWND);
                let class = CONTROL_CLASS
                    .encode_utf16()
                    .chain(Some(0))
                    .collect::<Vec<_>>();
                UnregisterClassW(
                    class.as_ptr(),
                    windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()),
                );
            }
        }
        hooks.mouse = 0;
        hooks.foreground = 0;
        hooks.location = 0;
        hooks.control = 0;
        hooks.session_registered = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_policy_checks_topmost_and_nonactivating_styles_independently() {
        let mut policy = WindowPolicy {
            tool: true,
            no_activate: true,
            topmost: true,
            shadow: false,
            standard: None,
        };
        let flags = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST;
        assert!(!policy_mismatch(policy, flags));
        for flag in [WS_EX_TOOLWINDOW, WS_EX_NOACTIVATE, WS_EX_TOPMOST] {
            assert!(policy_mismatch(policy, flags & !flag));
        }
        policy.topmost = false;
        assert!(policy_mismatch(policy, flags));
        assert!(!policy_mismatch(policy, flags & !WS_EX_TOPMOST));
    }
}
