//! Launch routing and requested/observed sign-in registration. No webview plugin access.
use serde::{Deserialize, Serialize};
use tauri_plugin_autostart::ManagerExt;

pub const LOGIN_ARGUMENT: &str = "--autostart";
pub const STARTUP_NAME: &str = "Codex Usage Widget";

#[derive(Default)]
pub struct LaunchRouter(std::sync::Mutex<(Option<crate::service::Service>, bool)>);
impl LaunchRouter {
    pub fn forward(&self, args: &[String]) {
        if background_launch(args) {
            return;
        }
        if let Ok(mut state) = self.0.lock() {
            if let Some(service) = &state.0 {
                open(service);
            } else {
                state.1 = true;
            }
        }
    }
    pub fn attach(&self, service: crate::service::Service) {
        if let Ok(mut state) = self.0.lock() {
            if state.1 {
                open(&service);
            }
            *state = (Some(service), false);
        }
    }
}
fn open(service: &crate::service::Service) {
    service.dispatch_reliable(crate::service::Action::Window(
        crate::windows::Input::Open {
            standard: true,
            settings: false,
        },
    ));
}

pub fn background_launch(args: impl IntoIterator<Item = impl AsRef<str>>) -> bool {
    args.into_iter()
        .skip(1)
        .any(|arg| arg.as_ref() == LOGIN_ARGUMENT)
}

pub fn initial_dashboard(first_launch: bool, tray_available: bool, background: bool) -> bool {
    first_launch || !tray_available || !background
}

/// Verify the pinned plugin acquired its native guard before starting background work.
/// Its Windows implementation can otherwise fall through when mutex/window setup fails.
pub fn verify_single_instance(app: &tauri::AppHandle) -> Result<(), &'static str> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId};
    let wide = |suffix: &str| {
        format!("{}{suffix}", app.config().identifier)
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>()
    };
    let class = wide("-sic");
    let title = wide("-siw");
    let hwnd = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
    let mut pid = 0;
    if !hwnd.is_null() {
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }
    }
    if pid == std::process::id() {
        Ok(())
    } else {
        Err("single-instance-guard-unavailable")
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Autostart {
    pub requested: bool,
    pub pending: bool,
    pub effective: Option<bool>,
    pub diagnostic: Option<String>,
}

impl Autostart {
    /// Ignore client-supplied observed fields. Persist this intent before external effects.
    pub fn request(&self, requested: bool) -> Self {
        Self {
            requested,
            pending: self.pending || requested != self.requested,
            ..self.clone()
        }
    }
}

trait Registration {
    fn enabled(&self) -> Result<bool, &'static str>;
    fn set(&self, enabled: bool) -> Result<(), &'static str>;
}

fn reconcile(previous: &Autostart, adapter: &impl Registration, apply: bool) -> Autostart {
    let mut next = previous.clone();
    next.diagnostic = None;
    if apply && next.pending {
        if let Err(code) = adapter.set(next.requested) {
            next.diagnostic = Some(code.into());
        }
    }
    match adapter.enabled() {
        Ok(effective) => {
            next.effective = Some(effective);
            if next.pending && effective == next.requested && next.diagnostic.is_none() {
                next.pending = false;
            }
            if effective != next.requested && next.diagnostic.is_none() {
                next.diagnostic = Some("autostart-mismatch".into());
            }
        }
        Err(code) => {
            next.effective = None;
            next.diagnostic = Some(code.into());
        }
    }
    next
}

struct Native<'a>(&'a tauri::AppHandle);
impl Registration for Native<'_> {
    fn enabled(&self) -> Result<bool, &'static str> {
        self.0
            .autolaunch()
            .is_enabled()
            .map_err(|_| "autostart-observation-failed")
    }
    fn set(&self, enabled: bool) -> Result<(), &'static str> {
        // The local auto-launch patch restricts registration to HKCU. Require the
        // user's normal desktop token so an elevated copy cannot alter a different profile.
        if !unelevated() {
            return Err("autostart-requires-standard-user");
        }
        if enabled {
            self.0.autolaunch().enable()
        } else {
            self.0.autolaunch().disable()
        }
        .map_err(|_| "autostart-registration-failed")
    }
}

pub fn autostart(app: &tauri::AppHandle, previous: &Autostart, apply: bool) -> Autostart {
    #[cfg(debug_assertions)]
    if crate::native_check::requested() {
        return Autostart {
            effective: Some(false),
            diagnostic: Some("autostart-validation-isolated".into()),
            ..previous.clone()
        };
    }
    reconcile(previous, &Native(app), apply)
}

fn unelevated() -> bool {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    let mut token = std::ptr::null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return false;
    }
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0;
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    } != 0;
    unsafe {
        CloseHandle(token);
    }
    ok && elevation.TokenIsElevated == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    struct Fake {
        enabled: Cell<bool>,
        fail: bool,
        calls: RefCell<Vec<bool>>,
    }
    impl Registration for Fake {
        fn enabled(&self) -> Result<bool, &'static str> {
            Ok(self.enabled.get())
        }
        fn set(&self, value: bool) -> Result<(), &'static str> {
            self.calls.borrow_mut().push(value);
            if self.fail {
                Err("autostart-registration-failed")
            } else {
                self.enabled.set(value);
                Ok(())
            }
        }
    }
    #[test]
    fn launch_routes_preserve_tray_recovery_and_ignore_non_flags() {
        assert!(background_launch(["widget.exe", LOGIN_ARGUMENT]));
        assert!(!background_launch([LOGIN_ARGUMENT]));
        assert!(!background_launch(["widget.exe", "--autostart=1"]));
        assert!(!initial_dashboard(false, true, true));
        assert!(initial_dashboard(true, true, true));
        assert!(initial_dashboard(false, false, true));
        assert!(initial_dashboard(false, true, false));
    }
    #[test]
    fn observe_external_disable_without_silently_overriding_it() {
        let api = Fake {
            enabled: Cell::new(false),
            fail: false,
            calls: RefCell::default(),
        };
        let previous = Autostart {
            requested: true,
            effective: Some(true),
            ..Default::default()
        };
        let next = reconcile(&previous, &api, false);
        assert!(next.requested);
        assert_eq!(next.effective, Some(false));
        assert_eq!(next.diagnostic.as_deref(), Some("autostart-mismatch"));
        assert!(api.calls.borrow().is_empty());
    }
    #[test]
    fn pending_intent_survives_failure_and_reconciles_idempotently() {
        let api = Fake {
            enabled: Cell::new(false),
            fail: true,
            calls: RefCell::default(),
        };
        let intent = Autostart::default().request(true);
        assert!(intent.pending);
        let failed = reconcile(&intent, &api, true);
        assert!(failed.pending);
        assert_eq!(failed.effective, Some(false));
        let api = Fake { fail: false, ..api };
        let success = reconcile(&failed, &api, true);
        assert!(!success.pending);
        assert_eq!(success.effective, Some(true));
        assert!(success.diagnostic.is_none());
        let observed = reconcile(&success, &api, true);
        assert_eq!(observed, success);
        assert_eq!(*api.calls.borrow(), vec![true, true]);
    }
    #[tokio::test(flavor = "current_thread")]
    async fn early_manual_launches_coalesce_until_the_service_is_attached() {
        let router = LaunchRouter::default();
        router.forward(&["widget.exe".into(), LOGIN_ARGUMENT.into()]);
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        router.forward(&["widget.exe".into()]);
        router.forward(&["widget.exe".into()]);
        router.attach(crate::service::Service(tx));
        let action = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            action,
            crate::service::Action::Window(crate::windows::Input::Open {
                standard: true,
                settings: false
            })
        ));
        assert!(rx.try_recv().is_err());
        router.forward(&["widget.exe".into(), LOGIN_ARGUMENT.into()]);
        assert!(rx.try_recv().is_err());
        router.forward(&["widget.exe".into()]);
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
                .await
                .unwrap()
                .is_some()
        );
    }
}
