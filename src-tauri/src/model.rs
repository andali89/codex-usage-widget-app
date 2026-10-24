use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
const CREDIT_STATUSES: &[&str] = &["available", "redeeming", "redeemed", "unknown"];

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DisplayMode {
    Floating,
    ScreenEdge,
    #[default]
    SystemTray,
    CompactHud,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EdgeSide {
    Left,
    #[default]
    Right,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum QuotaLabelStyle {
    #[default]
    Words,
    Badges,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum VisiblePeriods {
    #[default]
    Both,
    FiveHour,
    Weekly,
}

impl VisiblePeriods {
    #[cfg(debug_assertions)]
    pub fn includes(self, duration: i64) -> bool {
        matches!(
            (self, duration),
            (Self::Both, 300 | 10080) | (Self::FiveHour, 300) | (Self::Weekly, 10080)
        )
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum StorageLocation {
    #[default]
    Executable,
    AppData,
    Custom {
        directory: String,
    },
}

impl StorageLocation {
    pub fn validate(&self) -> Result<(), &'static str> {
        if let Self::Custom { directory } = self {
            if directory.len() > 32760
                || directory.chars().any(char::is_control)
                || !std::path::Path::new(directory).is_absolute()
            {
                return Err("invalid-storage-directory");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct WindowSettings {
    pub hover_enabled: bool,
    pub lock_position: bool,
    pub always_on_top: bool,
    pub suppress_fullscreen: bool,
    pub edge_side: EdgeSide,
    pub quota_label_style: QuotaLabelStyle,
    pub placements: BTreeMap<String, crate::geometry::Placement>,
    pub reset_countdowns: BTreeMap<String, ResetCountdowns>,
    pub visible_periods: BTreeMap<String, VisiblePeriods>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct ResetCountdowns {
    pub five_hour: bool,
    pub weekly: bool,
}
impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            hover_enabled: true,
            lock_position: false,
            always_on_top: false,
            suppress_fullscreen: true,
            edge_side: EdgeSide::Right,
            quota_label_style: QuotaLabelStyle::Words,
            reset_countdowns: BTreeMap::new(),
            visible_periods: BTreeMap::new(),
            placements: [
                ("floating", 24.0, 120.0),
                ("screen-edge", 0.0, 120.0),
                ("compact-hud", 24.0, 24.0),
            ]
            .into_iter()
            .map(|(key, x, y)| {
                (
                    key.into(),
                    crate::geometry::Placement {
                        monitor: None,
                        x,
                        y,
                    },
                )
            })
            .collect(),
        }
    }
}

impl DisplayMode {
    pub fn key(self) -> &'static str {
        match self {
            Self::Floating => "floating",
            Self::ScreenEdge => "screen-edge",
            Self::SystemTray => "system-tray",
            Self::CompactHud => "compact-hud",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub revision: u64,
    pub theme_preference: ThemePreference,
    pub mode: DisplayMode,
    pub codex_path: Option<String>,
    pub proxy_url: Option<String>,
    pub storage_location: StorageLocation,
    pub foreground_seconds: u64,
    pub compact_seconds: u64,
    pub background_seconds: u64,
    pub windows: WindowSettings,
    pub reminders: crate::reminders::Preferences,
    pub autostart: crate::lifecycle::Autostart,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 5,
            revision: 0,
            theme_preference: ThemePreference::System,
            mode: DisplayMode::SystemTray,
            codex_path: None,
            proxy_url: None,
            storage_location: StorageLocation::default(),
            foreground_seconds: 120,
            compact_seconds: 120,
            background_seconds: 120,
            windows: WindowSettings::default(),
            reminders: crate::reminders::Preferences::default(),
            autostart: crate::lifecycle::Autostart::default(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != 5 {
            return Err("settings-version");
        }
        self.storage_location.validate()?;
        if let Some(url) = &self.proxy_url {
            crate::proxy::validate(url)?;
        }
        if self
            .windows
            .visible_periods
            .keys()
            .any(|mode| !["screen-edge", "compact-hud"].contains(&mode.as_str()))
        {
            return Err("invalid-visible-periods");
        }
        if self
            .windows
            .reset_countdowns
            .keys()
            .any(|mode| !["floating", "screen-edge", "compact-hud"].contains(&mode.as_str()))
        {
            return Err("invalid-placement");
        }
        if self.windows.placements.len() > 3
            || self.windows.placements.iter().any(|(mode, p)| {
                !["floating", "screen-edge", "compact-hud"].contains(&mode.as_str())
                    || !p.validate()
            })
        {
            return Err("invalid-placement");
        }
        if !(15..=120).contains(&self.foreground_seconds)
            || !(30..=300).contains(&self.compact_seconds)
            || !(60..=600).contains(&self.background_seconds)
            || self.foreground_seconds > self.compact_seconds
            || self.compact_seconds > self.background_seconds
        {
            return Err("invalid-refresh-intervals");
        }
        if let Some(path) = &self.codex_path {
            if path.len() > 32760 || path.contains(['\n', '\r', '\0']) {
                return Err("invalid-executable-path");
            }
        }
        self.reminders.validate()
    }
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub theme_preference: ThemePreference,
    pub resolved_theme: String,
    pub high_contrast: bool,
    pub transparency_allowed: bool,
    pub reduced_motion: bool,
    pub text_scale: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub key: String,
    pub limit_id: String,
    pub label: String,
    pub used_percent: Option<f64>,
    pub remaining_percent: Option<f64>,
    pub duration_minutes: Option<i64>,
    pub resets_at_ms: Option<i64>,
    pub validated_at_ms: Option<i64>,
    pub quality: String,
    pub last_known: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetCredit {
    // The backend credit ID is deliberately not exposed: this app cannot redeem credits.
    pub status: String,
    pub expires_at_ms: Option<i64>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub account_context_key: Option<String>,
    pub connection_generation: u64,
    pub selected_limit_id: Option<String>,
    pub windows: Vec<QuotaWindow>,
    pub reset_credits_available: Option<u64>,
    pub reset_credits: Option<Vec<ResetCredit>>,
    pub plan_type: Option<String>,
    pub ordinary_usage_allowed: Option<bool>,
    pub additional_limits: bool,
    pub last_attempt_at_ms: Option<i64>,
    pub last_success_at_ms: Option<i64>,
    pub next_refresh_at_ms: Option<i64>,
    pub refresh_in_flight: bool,
    pub connection: String,
    pub freshness: String,
    pub error_code: Option<String>,
    pub codex_cli_version: Option<String>,
    pub consecutive_unavailable_reads: u32,
    pub missing_window_reads: BTreeMap<String, u32>,
}

/// The event loop wakes at this deadline; it need not wait for a minute tick.
/// Invalid retained fields already carry last-known status and cannot extend freshness.
pub fn freshness_deadline(usage: &Usage, interval_ms: u64) -> Option<i64> {
    if usage.freshness != "fresh" {
        return None;
    }
    let latest = usage.last_success_at_ms?;
    let oldest = usage
        .windows
        .iter()
        .filter(|w| w.quality == "valid" && !w.last_known)
        .filter_map(|w| w.validated_at_ms)
        .fold(latest, i64::min);
    let ttl = interval_ms
        .saturating_mul(2)
        .saturating_add(15_000)
        .max(90_000)
        .min(i64::MAX as u64) as i64;
    Some(oldest.saturating_add(ttl))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub process_epoch: String,
    pub revision: u64,
    pub settings: Settings,
    pub appearance: Appearance,
    pub usage: Usage,
    pub storage_diagnostic: Option<String>,
    pub storage: crate::storage_location::Status,
    pub tray_available: bool,
    pub dashboard_visible: bool,
    pub presentation: crate::interaction::Presentation,
    pub window_diagnostic: Option<String>,
    pub tooltip_fallback: bool,
    pub effective_refresh_seconds: u64,
    pub monitors: Vec<MonitorChoice>,
    pub reminders: crate::reminders::Snapshot,
    pub dashboard_request: Option<DashboardRequest>,
    pub first_launch: bool,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardRequest {
    pub sequence: u64,
    pub page: DashboardPage,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DashboardPage {
    Overview,
    Settings,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorChoice {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

pub fn seconds_to_ms(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .filter(|s| (0..=253_402_300_799).contains(s))
        .and_then(|s| s.checked_mul(1000))
}

fn period_label(duration: Option<i64>, slot: &str) -> String {
    match duration {
        Some(300) => "5-hour".into(),
        Some(10080) => "Weekly".into(),
        Some(n) if n % 1440 == 0 => format!("{}-day", n / 1440),
        Some(n) if n % 60 == 0 => format!("{}-hour", n / 60),
        Some(n) => format!("{n}-minute"),
        None => format!(
            "{} window (duration unavailable)",
            if slot == "primary" {
                "Primary"
            } else {
                "Secondary"
            }
        ),
    }
}

// Only this adapter knows the upstream wire shape. Unknown fields are never displayed.
pub fn normalize(raw: &Value, at_ms: i64, previous: &Usage) -> Usage {
    let family = raw.get("rateLimits").filter(|v| v.is_object()).or_else(|| {
        raw.pointer("/rateLimitsByLimitId/codex")
            .filter(|v| v.is_object())
    });
    let mut result = Usage {
        last_success_at_ms: Some(at_ms),
        connection: "ready".into(),
        freshness: "fresh".into(),
        ..Usage::default()
    };
    // Availability is independent of whether percentages or a family were returned.
    result.ordinary_usage_allowed = raw.get("ordinaryUsageAllowed").and_then(Value::as_bool);
    result.additional_limits = raw
        .get("rateLimitsByLimitId")
        .and_then(Value::as_object)
        .is_some_and(|m| {
            m.keys().any(|k| {
                k != family
                    .and_then(|f| f.get("limitId"))
                    .and_then(Value::as_str)
                    .unwrap_or("codex")
            })
        });
    let Some(family) = family else {
        result.consecutive_unavailable_reads =
            previous.consecutive_unavailable_reads.saturating_add(1);
        result.missing_window_reads = previous
            .missing_window_reads
            .iter()
            .map(|(key, count)| (key.clone(), count.saturating_add(1)))
            .collect();
        for window in &previous.windows {
            if let Some((slot, _)) = window.key.rsplit_once('/') {
                result.missing_window_reads.entry(slot.into()).or_insert(1);
            }
        }
        if result.consecutive_unavailable_reads < 3 {
            result.windows = previous.windows.clone();
            for window in &mut result.windows {
                window.last_known = true;
                window.quality = "missing".into();
            }
        }
        result.freshness = if result.windows.is_empty() {
            "empty"
        } else {
            "stale"
        }
        .into();
        return result;
    };
    let limit_id = family
        .get("limitId")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .unwrap_or("codex");
    result.selected_limit_id = Some(limit_id.into());
    result.plan_type = family
        .get("planType")
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 64)
        .map(str::to_owned);
    result.reset_credits_available = raw
        .pointer("/rateLimitResetCredits/availableCount")
        .and_then(Value::as_u64)
        .filter(|n| *n <= i64::MAX as u64);
    result.reset_credits = raw
        .pointer("/rateLimitResetCredits/credits")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .take(100)
                .filter_map(|c| {
                    let status = c.get("status")?.as_str()?;
                    if !CREDIT_STATUSES.contains(&status) {
                        return None;
                    }
                    Some(ResetCredit {
                        status: status.into(),
                        expires_at_ms: c.get("expiresAt").and_then(seconds_to_ms),
                    })
                })
                .collect()
        });
    for slot in ["primary", "secondary"] {
        let value = family.get(slot).unwrap_or(&Value::Null);
        let duration = value
            .get("windowDurationMins")
            .and_then(Value::as_i64)
            .filter(|n| *n > 0 && *n <= 5_256_000);
        let percent = value
            .get("usedPercent")
            .and_then(Value::as_f64)
            .filter(|n| n.is_finite() && (0.0..=100.0).contains(n));
        let key = format!(
            "{limit_id}/{slot}/{}",
            duration.map_or_else(|| "unknown".into(), |n| n.to_string())
        );
        let mut window = QuotaWindow {
            key,
            limit_id: limit_id.into(),
            label: period_label(duration, slot),
            used_percent: percent,
            remaining_percent: percent.map(|p| 100.0 - p),
            duration_minutes: duration,
            resets_at_ms: value.get("resetsAt").and_then(seconds_to_ms),
            validated_at_ms: percent.map(|_| at_ms),
            quality: if value.is_null() {
                "missing"
            } else if percent.is_some() {
                "valid"
            } else {
                "invalid"
            }
            .into(),
            last_known: false,
        };
        if window.quality != "valid" {
            let missing_key = format!("{limit_id}/{slot}");
            let count = previous
                .missing_window_reads
                .get(&missing_key)
                .copied()
                .unwrap_or(0)
                .saturating_add(1);
            result.missing_window_reads.insert(missing_key, count);
            if let Some(old) = previous.windows.iter().find(|w| {
                w.key == window.key
                    || (duration.is_none() && w.key.starts_with(&format!("{limit_id}/{slot}/")))
            }) {
                window.key = old.key.clone();
                window.label = old.label.clone();
                window.duration_minutes = old.duration_minutes;
                if count < 3 && old.remaining_percent.is_some() {
                    window.used_percent = old.used_percent;
                    window.remaining_percent = old.remaining_percent;
                    window.resets_at_ms = old.resets_at_ms;
                    window.validated_at_ms = old.validated_at_ms;
                    window.last_known = true;
                }
            }
        }
        result.windows.push(window);
    }
    if result.windows.iter().all(|w| w.remaining_percent.is_none()) {
        result.freshness = "empty".into();
    }
    if result.windows.iter().all(|w| w.quality != "valid") {
        result.consecutive_unavailable_reads =
            previous.consecutive_unavailable_reads.saturating_add(1);
    }
    if result.windows.iter().any(|w| w.quality == "invalid") {
        result.error_code = Some("invalid-quota-field".into());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn connection_storage_and_visible_period_preferences_validate_and_round_trip() {
        let mut settings = Settings::default();
        assert_eq!(settings.storage_location, StorageLocation::Executable);
        assert!(settings.proxy_url.is_none());
        assert!(settings.windows.visible_periods.is_empty());
        settings.proxy_url = Some("http://127.0.0.1:7890".into());
        settings
            .windows
            .visible_periods
            .insert("screen-edge".into(), VisiblePeriods::FiveHour);
        settings
            .windows
            .visible_periods
            .insert("compact-hud".into(), VisiblePeriods::Weekly);
        let copy: Settings =
            serde_json::from_value(serde_json::to_value(&settings).unwrap()).unwrap();
        copy.validate().unwrap();
        settings.storage_location = StorageLocation::Custom {
            directory: "relative/path".into(),
        };
        assert_eq!(settings.validate(), Err("invalid-storage-directory"));
        settings.storage_location = StorageLocation::AppData;
        settings
            .windows
            .visible_periods
            .insert("floating".into(), VisiblePeriods::Both);
        assert_eq!(settings.validate(), Err("invalid-visible-periods"));
    }
    #[test]
    fn quota_label_style_defaults_and_round_trips_in_current_schema() {
        let old: Settings =
            serde_json::from_value(serde_json::json!({"schemaVersion":5,"windows":{}})).unwrap();
        assert_eq!(old.windows.quota_label_style, QuotaLabelStyle::Words);
        for style in [QuotaLabelStyle::Words, QuotaLabelStyle::Badges] {
            let mut settings = old.clone();
            settings.windows.quota_label_style = style;
            let copy: Settings =
                serde_json::from_value(serde_json::to_value(settings).unwrap()).unwrap();
            assert_eq!(copy.windows.quota_label_style, style);
            assert!(copy.validate().is_ok());
        }
        assert!(serde_json::from_value::<Settings>(
            serde_json::json!({"windows":{"quotaLabelStyle":"invalid"}})
        )
        .is_err());
    }
    #[test]
    fn countdown_options_default_for_old_settings_and_round_trip_per_mode() {
        let old: Settings =
            serde_json::from_value(serde_json::json!({"schemaVersion":5,"windows":{}})).unwrap();
        assert!(old.windows.reset_countdowns.is_empty());
        let mut next = old;
        next.windows.reset_countdowns.insert(
            "floating".into(),
            ResetCountdowns {
                five_hour: true,
                weekly: false,
            },
        );
        let copy: Settings = serde_json::from_value(serde_json::to_value(&next).unwrap()).unwrap();
        assert!(copy.validate().is_ok());
        assert!(copy.windows.reset_countdowns["floating"].five_hour);
        assert!(!copy.windows.reset_countdowns["floating"].weekly);
        assert!(!copy.windows.reset_countdowns.contains_key("compact-hud"));
        next.windows
            .reset_countdowns
            .insert("unknown".into(), ResetCountdowns::default());
        assert!(next.validate().is_err());
    }
    #[test]
    fn missing_window_retains_marked_values_twice_then_clears_and_recovers() {
        let good = serde_json::json!({"rateLimits":{"primary":{"usedPercent":26,"windowDurationMins":300,"resetsAt":1000},"secondary":{"usedPercent":10,"windowDurationMins":10080}}});
        let missing = serde_json::json!({"rateLimits":{"secondary":{"usedPercent":10,"windowDurationMins":10080}}});
        let mut usage = normalize(&good, 1, &Usage::default());
        for at in [2, 3] {
            usage = normalize(&missing, at, &usage);
            assert_eq!(usage.windows[0].remaining_percent, Some(74.0));
            assert_eq!(usage.windows[0].duration_minutes, Some(300));
            assert_eq!(usage.windows[0].resets_at_ms, Some(1_000_000));
            assert_eq!(usage.windows[0].validated_at_ms, Some(1));
            assert!(usage.windows[0].last_known);
            assert_eq!(usage.windows[0].quality, "missing");
        }
        usage = normalize(&missing, 4, &usage);
        assert_eq!(usage.windows[0].remaining_percent, None);
        usage = normalize(&good, 5, &usage);
        assert!(!usage.windows[0].last_known);
        assert!(usage.missing_window_reads.is_empty());
        usage = normalize(&missing, 6, &usage);
        assert_eq!(usage.windows[0].remaining_percent, Some(74.0));
    }
    #[test]
    fn absent_family_does_not_erase_last_known_data_on_a_single_read() {
        let mut usage = normalize(
            &serde_json::json!({"rateLimits":{"primary":{"usedPercent":26,"windowDurationMins":300}}}),
            1,
            &Usage::default(),
        );
        for at in [2, 3] {
            usage = normalize(&serde_json::json!({}), at, &usage);
            assert_eq!(usage.windows[0].remaining_percent, Some(74.0));
            assert!(usage.windows[0].last_known);
            assert_eq!(usage.freshness, "stale");
        }
        usage = normalize(&serde_json::json!({}), 4, &usage);
        assert!(usage.windows.is_empty());
        assert_eq!(usage.consecutive_unavailable_reads, 3);
    }
    #[test]
    fn freshness_expires_at_the_oldest_valid_observation_without_waiting_for_polling() {
        let mut u = normalize(
            &serde_json::json!({"rateLimits":{"primary":{"usedPercent":26,"windowDurationMins":300}}}),
            1000,
            &Usage::default(),
        );
        assert_eq!(freshness_deadline(&u, 30_000), Some(91_000));
        u.refresh_in_flight = true;
        assert_eq!(freshness_deadline(&u, 30_000), Some(91_000));
        u.windows[0].validated_at_ms = Some(500);
        assert_eq!(freshness_deadline(&u, 30_000), Some(90_500));
        u.windows[0].last_known = true;
        assert_eq!(freshness_deadline(&u, 30_000), Some(91_000));
        assert_eq!(freshness_deadline(&u, 120_000), Some(256_000));
        u.freshness = "stale".into();
        assert_eq!(freshness_deadline(&u, 30_000), None);
        assert_eq!(freshness_deadline(&Usage::default(), 30_000), None);
    }
    use serde_json::json;
    #[test]
    fn percentages_and_durations_are_not_inferred() {
        let u = normalize(
            &json!({"rateLimits":{"primary":{"usedPercent":26,"windowDurationMins":300},"secondary":{"usedPercent":99.5}}}),
            7,
            &Usage::default(),
        );
        assert_eq!(u.windows[0].remaining_percent, Some(74.0));
        assert_eq!(u.windows[0].label, "5-hour");
        assert_eq!(u.windows[1].remaining_percent, Some(0.5));
        assert!(u.windows[1].label.contains("duration unavailable"));
        assert_eq!(u.ordinary_usage_allowed, None);
    }
    #[test]
    fn missing_invalid_and_family_selection() {
        for v in [json!(-1), json!(101), json!("20"), Value::Null] {
            let u = normalize(
                &json!({"rateLimits":{"primary":{"usedPercent":v},"secondary":null}}),
                1,
                &Usage::default(),
            );
            assert_eq!(u.windows[0].remaining_percent, None);
            assert_eq!(u.windows[1].quality, "missing");
        }
        let u = normalize(
            &json!({"rateLimitsByLimitId":{"other":{"primary":{"usedPercent":0}}}}),
            1,
            &Usage::default(),
        );
        assert!(u.windows.is_empty());
        assert!(u.additional_limits);
    }
    #[test]
    fn invalid_retains_only_marked_same_mapping_cache() {
        let first = normalize(
            &json!({"rateLimits":{"primary":{"usedPercent":26,"windowDurationMins":300}}}),
            1,
            &Usage::default(),
        );
        let next = normalize(
            &json!({"rateLimits":{"primary":{"usedPercent":101,"windowDurationMins":300}}}),
            2,
            &first,
        );
        assert_eq!(next.windows[0].remaining_percent, Some(74.0));
        assert!(next.windows[0].last_known);
        assert_eq!(next.windows[0].validated_at_ms, Some(1));
        let next = normalize(
            &json!({"rateLimits":{"primary":{"usedPercent":101,"windowDurationMins":60}}}),
            3,
            &first,
        );
        assert_eq!(next.windows[0].remaining_percent, None);
    }
    #[test]
    fn optional_fields_and_timestamp_bounds() {
        let u = normalize(
            &json!({"rateLimits":{},"ordinaryUsageAllowed":false,"rateLimitResetCredits":{"availableCount":-1},"credits":{"balance":"500"}}),
            1,
            &Usage::default(),
        );
        assert_eq!(u.ordinary_usage_allowed, Some(false));
        assert_eq!(u.reset_credits_available, None);
        assert_eq!(
            seconds_to_ms(&json!(1_700_000_000)),
            Some(1_700_000_000_000)
        );
        assert_eq!(seconds_to_ms(&json!(i64::MAX)), None);
        let no_windows = normalize(&json!({"ordinaryUsageAllowed":false}), 1, &Usage::default());
        assert_eq!(no_windows.ordinary_usage_allowed, Some(false));
    }
    #[test]
    fn credit_status_allowlist_matches_pinned_schema() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../schemas/codex/0.163.0/v2/GetAccountRateLimitsResponse.json"
        ))
        .unwrap();
        let statuses = schema
            .pointer("/definitions/RateLimitResetCreditStatus/enum")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(CREDIT_STATUSES, statuses);
    }
    #[test]
    fn settings_boundaries_and_defaults() {
        let mut s = Settings::default();
        assert_eq!(s.theme_preference, ThemePreference::System);
        assert_eq!(s.mode, DisplayMode::SystemTray);
        assert!(s.proxy_url.is_none());
        assert_eq!(
            (
                s.foreground_seconds,
                s.compact_seconds,
                s.background_seconds
            ),
            (120, 120, 120)
        );
        assert!(s.validate().is_ok());
        s.foreground_seconds = 5;
        assert!(s.validate().is_err());
        s.foreground_seconds = 121;
        assert!(s.validate().is_err());
        s.foreground_seconds = 120;
        assert!(s.validate().is_ok());
        s.compact_seconds = 60;
        assert!(s.validate().is_err());
        s.foreground_seconds = 60;
        assert!(s.validate().is_ok());
        s.compact_seconds = 121;
        assert!(s.validate().is_err());
    }
}
