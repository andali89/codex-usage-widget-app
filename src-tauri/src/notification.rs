//! Ordinary installed notifications via Tauri. A queued plugin call is unconfirmed;
//! neither its desktop permission stub nor its queue result proves OS display.
use crate::{
    model::Usage,
    reminders::{Category, Delivery, Outcome, Preferences},
};
use tauri::Manager;
use tauri_plugin_notification::NotificationExt;

pub struct Payload {
    pub title: String,
    pub body: String,
    pub silent: bool,
}
fn percent(value: f64) -> String {
    if value > 0.0 && value < 1.0 {
        "<1%".into()
    } else {
        format!("{:.0}%", value)
    }
}
pub fn payload(delivery: &Delivery, prefs: &Preferences, usage: &Usage, now: i64) -> Payload {
    let fresh = usage.freshness == "fresh"
        && usage
            .last_success_at_ms
            .is_some_and(|at| now >= at && now - at <= 60_000);
    let mut lines = Vec::new();
    if delivery.events.iter().any(|c| c.category == Category::Test) {
        lines.push("This is a notification test. No usage thresholds were evaluated.".into());
    } else if prefs.detailed {
        for c in &delivery.events {
            let detail = c
                .remaining
                .map(|r| format!("{} remaining", percent(r)))
                .unwrap_or_else(|| "Usage data unavailable".into());
            lines.push(match c.category {
                Category::Low => format!("{}: {detail}", c.label),
                Category::Approaching => format!(
                    "{}: Expected reset in {} min",
                    c.label,
                    c.expires_at
                        .map(|at| ((at.saturating_sub(now)).max(0) as u64).div_ceil(60_000))
                        .unwrap_or(0)
                ),
                Category::Recovery => format!("Quota increased · {}: {detail}", c.label),
                Category::Availability => "Usage available again (confirmed by Codex)".into(),
                Category::Scheduled => {
                    if fresh {
                        let values = usage
                            .windows
                            .iter()
                            .filter(|w| w.quality == "valid" && !w.last_known)
                            .filter_map(|w| {
                                w.remaining_percent
                                    .map(|r| format!("{}: {} remaining", w.label, percent(r)))
                            })
                            .collect::<Vec<_>>();
                        format!(
                            "{} · {}",
                            c.label,
                            if values.is_empty() {
                                "Usage data unavailable/out of date".into()
                            } else {
                                values.join("; ")
                            }
                        )
                    } else {
                        format!("{} · Usage data unavailable/out of date", c.label)
                    }
                }
                Category::Test => "Notification test".into(),
            });
        }
        if delivery
            .events
            .iter()
            .any(|c| c.category == Category::Recovery)
        {
            let exhausted = usage
                .windows
                .iter()
                .filter(|w| {
                    w.quality == "valid" && !w.last_known && w.remaining_percent == Some(0.0)
                })
                .map(|w| w.label.as_str())
                .collect::<Vec<_>>();
            if !exhausted.is_empty() {
                lines.push(format!("Still exhausted: {}", exhausted.join(", ")));
            }
            if usage.ordinary_usage_allowed == Some(false) {
                lines.push("Ordinary usage remains blocked".into());
            } else if !fresh
                || usage
                    .windows
                    .iter()
                    .any(|w| w.quality != "valid" || w.last_known)
            {
                lines.push("Other usage conditions are unavailable/out of date".into());
            }
        }
    } else {
        lines.push("A Codex usage reminder is ready.".into());
        if delivery
            .events
            .iter()
            .any(|c| c.category == Category::Scheduled)
            && (!fresh
                || !usage.windows.iter().any(|w| {
                    w.quality == "valid" && !w.last_known && w.remaining_percent.is_some()
                }))
        {
            lines.push("Usage data unavailable/out of date.".into());
        }
    }
    lines.push("Open the Codex tray icon for details.".into());
    Payload {
        title: "Codex Usage Widget".into(),
        body: lines.join("\n"),
        silent: prefs.silent,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Availability {
    InstallRequired,
    Enabled,
    ApplicationDisabled,
    UserDisabled,
    PolicyDisabled,
    ManifestDisabled,
    Unknown,
}
impl Availability {
    fn from_setting(setting: i32) -> Self {
        match setting {
            0 => Self::Enabled,
            1 => Self::ApplicationDisabled,
            2 => Self::UserDisabled,
            3 => Self::PolicyDisabled,
            4 => Self::ManifestDisabled,
            _ => Self::Unknown,
        }
    }
    fn diagnostic(self) -> Option<&'static str> {
        match self {
            Self::InstallRequired => Some("notifications-install-required"),
            Self::Enabled => None,
            Self::ApplicationDisabled => Some("notifications-disabled-for-app"),
            Self::UserDisabled => Some("notifications-disabled-for-user"),
            Self::PolicyDisabled => Some("notifications-disabled-by-policy"),
            Self::ManifestDisabled => Some("notifications-identity-unavailable"),
            Self::Unknown => Some("notifications-status-unknown"),
        }
    }
}

pub struct Adapter {
    availability: Availability,
    last_error: Option<&'static str>,
}
impl Adapter {
    pub fn new(app: &tauri::AppHandle) -> Self {
        Self {
            availability: inspect(app),
            last_error: None,
        }
    }
    pub fn ready(&self) -> bool {
        self.availability == Availability::Enabled
    }
    pub fn diagnostic(&self) -> Option<String> {
        self.availability
            .diagnostic()
            .or(self.last_error)
            .map(str::to_owned)
    }
    pub fn reconcile(&mut self, app: &tauri::AppHandle) {
        self.availability = inspect(app);
    }
    pub fn submit(
        &mut self,
        app: &tauri::AppHandle,
        delivery: &Delivery,
        preferences: &Preferences,
        usage: &Usage,
        now: i64,
    ) -> Outcome {
        self.reconcile(app);
        if !self.ready() {
            return Outcome::DefinitelyNotSubmitted;
        }
        let content = payload(delivery, preferences, usage, now);
        let mut builder = app
            .notification()
            .builder()
            .title(content.title)
            .body(content.body);
        // The pinned Windows backend emits <audio silent="true"/> when sound is absent.
        if !content.silent {
            builder = builder.sound("Default");
        }
        if builder.show().is_err() {
            // Source-validated: an error here precedes the background submission task.
            self.last_error = Some("notifications-preparation-failed");
            Outcome::DefinitelyNotSubmitted
        } else {
            self.last_error = Some("notifications-queued-unconfirmed");
            Outcome::Unknown
        }
    }
}

fn inspect(app: &tauri::AppHandle) -> Availability {
    if !installed(app) {
        return Availability::InstallRequired;
    }
    use windows_bindings::{
        core::HSTRING,
        Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
        UI::Notifications::ToastNotificationManager,
    };
    // A Tauri UI thread may already be STA; an existing apartment is also usable.
    let initialization = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
    let balance = initialization.is_ok();
    if let Err(error) = initialization {
        if error.code().0 != 0x80010106_u32 as i32 {
            trace_status_failure("initialize", error.code().0);
            return Availability::Unknown;
        }
    }
    let setting = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(
        &app.config().identifier,
    ))
    .inspect_err(|error| trace_status_failure("create-notifier", error.code().0))
    .and_then(|notifier| {
        notifier
            .Setting()
            .inspect_err(|error| trace_status_failure("read-setting", error.code().0))
    });
    if balance {
        unsafe {
            RoUninitialize();
        }
    }
    setting
        .map(|value| Availability::from_setting(value.0))
        .unwrap_or(Availability::Unknown)
}

fn trace_status_failure(stage: &str, code: i32) {
    #[cfg(debug_assertions)]
    if crate::native_check::notifications_requested() {
        eprintln!(
            "native-check notification-status stage={stage} hresult={:08X}",
            code as u32
        );
    }
    #[cfg(not(debug_assertions))]
    let _ = (stage, code);
}

fn installed(app: &tauri::AppHandle) -> bool {
    if tauri::utils::platform::bundle_type() != Some(tauri::utils::config::BundleType::Nsis) {
        return false;
    }
    #[cfg(debug_assertions)]
    if crate::native_check::requested() && !crate::native_check::notifications_requested() {
        return false;
    }
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let key = wide(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Codex Usage Widget");
    let name = wide("InstallLocation");
    let mut buffer = vec![0_u16; 32768];
    let mut bytes = (buffer.len() * 2) as u32;
    if unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    } != 0
    {
        return false;
    }
    let length = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    let Ok(location) = String::from_utf16(&buffer[..length]) else {
        return false;
    };
    let expected = std::path::Path::new(location.trim_matches('"'))
        .join("Usage.exe")
        .canonicalize();
    let actual = tauri::utils::platform::current_exe().and_then(|p| p.canonicalize());
    let shortcut = app
        .path()
        .data_dir()
        .map(|p| p.join(r"Microsoft\Windows\Start Menu\Programs\Codex Usage Widget.lnk"));
    expected.ok().zip(actual.ok()).is_some_and(|(a, b)| a == b)
        && shortcut.is_ok_and(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reminders::{Candidate, Status};
    #[test]
    fn windows_permission_blocks_and_unknown_values_never_enable_delivery() {
        assert_eq!(Availability::from_setting(0), Availability::Enabled);
        for value in [1, 2, 3, 4, 5, -1] {
            let status = Availability::from_setting(value);
            assert_ne!(status, Availability::Enabled);
            assert!(status.diagnostic().is_some());
        }
        assert!(Availability::InstallRequired.diagnostic().is_some());
    }
    fn delivery(category: Category) -> Delivery {
        Delivery {
            id: 1,
            events: vec![Candidate {
                rule_key: "private-binding".into(),
                event_keys: vec!["private-event".into()],
                category,
                binding: Some("private-account".into()),
                episode: 1,
                thresholds: vec![0],
                label: "5-hour".into(),
                remaining: Some(0.4),
                eligible_at: 0,
                observed_at: 0,
                expires_at: None,
                snapshot_revision: 1,
                suppression: None,
            }],
            at_ms: 5000,
            status: Status::Claimed,
            reason: None,
            attempts: 1,
            retry_at: None,
        }
    }
    #[test]
    fn generic_default_is_silent_and_omits_private_bindings_and_quota_details() {
        let p = Preferences::default();
        let content = payload(&delivery(Category::Low), &p, &Usage::default(), 5000);
        assert!(content.silent);
        assert!(!content.body.contains("private"));
        assert!(!content.body.contains("%"));
    }
    #[test]
    fn detail_preserves_fractional_quota_and_stale_schedules_are_neutral() {
        let p = Preferences {
            detailed: true,
            ..Default::default()
        };
        let content = payload(&delivery(Category::Low), &p, &Usage::default(), 5000);
        assert!(content.body.contains("<1% remaining"));
        assert!(!content.body.contains("private"));
        let content = payload(&delivery(Category::Scheduled), &p, &Usage::default(), 5000);
        assert!(content.body.contains("Usage data unavailable/out of date"));
        assert!(!content.body.contains("%"));
    }
    #[test]
    fn manual_test_payload_stays_generic_even_when_details_are_enabled() {
        let prefs = Preferences {
            detailed: true,
            ..Default::default()
        };
        let content = payload(&delivery(Category::Test), &prefs, &Usage::default(), 5000);
        assert!(content.body.contains("notification test"));
        assert!(!content.body.contains("%"));
        assert!(!content.body.contains("private"));
        assert!(content.silent);
    }
}
