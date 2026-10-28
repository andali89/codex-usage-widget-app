//! Uniform compact-widget scaling. Saved preferences are never changed by screen fit.
use crate::{
    model::{DisplayMode, QuotaLabelStyle, Settings, VisiblePeriods},
    widget_appearance::Issue,
};
use serde::Serialize;
use std::collections::BTreeMap;
use tauri::{AppHandle, Manager};

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaleState {
    pub requested_percent: u16,
    pub effective_percent: u16,
    pub max_percent: u16,
    // Client-space budget before the user's snap gap; logical pixels at this DPI.
    pub available_width: f64,
    pub available_height: f64,
    pub fallback_reason: Option<&'static str>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorBudget {
    pub available_width: f64,
    pub available_height: f64,
}

pub fn requested(settings: &Settings, mode: DisplayMode) -> u16 {
    match mode {
        DisplayMode::ScreenEdge => settings.widget_appearance.edge_scale_percent,
        DisplayMode::CompactHud => settings.widget_appearance.hud_scale_percent,
        _ => 100,
    }
}

/// Mirrors compactLayout's current geometry, reserving the visible-warning space.
pub fn base_size(settings: &Settings, mode: DisplayMode) -> (f64, f64) {
    let periods = settings
        .windows
        .visible_periods
        .get(mode.key())
        .copied()
        .unwrap_or_default();
    let single = periods != VisiblePeriods::Both;
    let badges = settings.windows.quota_label_style == QuotaLabelStyle::Badges;
    let resets = settings.windows.reset_countdowns.get(mode.key());
    let count = f64::from(
        u8::from(periods != VisiblePeriods::Weekly && resets.is_some_and(|r| r.five_hour))
            + u8::from(periods != VisiblePeriods::FiveHour && resets.is_some_and(|r| r.weekly)),
    );
    if mode == DisplayMode::ScreenEdge {
        (
            54.0,
            (if badges {
                if single {
                    100.0
                } else {
                    150.0
                }
            } else if single {
                120.0
            } else {
                190.0
            }) + 24.0 * count
                + 18.0,
        )
    } else {
        (
            (if badges {
                if single {
                    102.0
                } else {
                    164.0
                }
            } else if single {
                120.0
            } else {
                200.0
            }) + 4.0
                + 58.0 * count
                + 28.0,
            38.0,
        )
    }
}

pub fn fit(
    requested: u16,
    base: (f64, f64),
    available: (f64, f64),
    text_scale: u32,
    gap: u8,
) -> ScaleState {
    let text = f64::from(text_scale.max(1)) / 100.0;
    let gap = f64::from(gap) * 2.0;
    let max_percent = (((available.0 - gap).max(0.0) / (base.0 * text))
        .min((available.1 - gap).max(0.0) / (base.1 * text))
        * 100.0
        + 1e-8)
        .floor()
        .clamp(0.0, 300.0) as u16;
    let fallback = requested > max_percent;
    ScaleState {
        requested_percent: requested,
        effective_percent: if fallback {
            requested.min(100)
        } else {
            requested
        },
        max_percent,
        available_width: available.0,
        available_height: available.1,
        fallback_reason: fallback.then_some("screen-too-small"),
    }
}

/// Budgets for every monitor let Settings validate an unsaved monitor selection.
/// The empty key represents the primary-monitor fallback.
pub fn monitor_budgets(app: &AppHandle) -> BTreeMap<String, MonitorBudget> {
    let shell = app.get_webview_window("shell");
    let frame = shell
        .as_ref()
        .and_then(|w| {
            let dpi = w.scale_factor().ok()?;
            let insets = crate::native_windows::content_outer_size(w, (0.0, 0.0), 1.0).ok()?;
            Some((insets.0 / dpi, insets.1 / dpi))
        })
        .unwrap_or((32.0, 32.0)); // Conservative until a shell provides measured insets.
    let primary = app.primary_monitor().ok().flatten();
    let mut budgets = BTreeMap::new();
    for m in app.available_monitors().unwrap_or_default() {
        let dpi = m.scale_factor();
        let area = m.work_area();
        let (safety, _) = crate::native_windows::taskbar_safety(crate::geometry::Rect {
            x: m.position().x as f64,
            y: m.position().y as f64,
            width: m.size().width as f64,
            height: m.size().height as f64,
        });
        let budget = MonitorBudget {
            available_width: f64::from(area.size.width) / dpi
                - frame.0
                - 8.0 * f64::from(u8::from(safety[0]) + u8::from(safety[2])),
            available_height: f64::from(area.size.height) / dpi
                - frame.1
                - 8.0 * f64::from(u8::from(safety[1]) + u8::from(safety[3])),
        };
        if primary
            .as_ref()
            .is_some_and(|p| p.position() == m.position())
        {
            budgets.insert(String::new(), budget.clone());
        }
        if let Some(name) = m.name() {
            budgets.insert(name.clone(), budget);
        }
    }
    budgets
}

pub fn resolve_with_budgets(
    settings: &Settings,
    text_scale: u32,
    budgets: &BTreeMap<String, MonitorBudget>,
) -> BTreeMap<String, ScaleState> {
    [DisplayMode::ScreenEdge, DisplayMode::CompactHud]
        .into_iter()
        .map(|mode| {
            let name = settings
                .windows
                .placements
                .get(mode.key())
                .and_then(|p| p.monitor.as_deref())
                .unwrap_or("");
            let available = budgets
                .get(name)
                .or_else(|| budgets.get(""))
                .map_or((0.0, 0.0), |b| (b.available_width, b.available_height));
            (
                mode.key().into(),
                fit(
                    requested(settings, mode),
                    base_size(settings, mode),
                    available,
                    text_scale,
                    settings.windows.snap_gap_logical_px,
                ),
            )
        })
        .collect()
}

pub fn resolve(
    app: &AppHandle,
    settings: &Settings,
    text_scale: u32,
) -> BTreeMap<String, ScaleState> {
    resolve_with_budgets(settings, text_scale, &monitor_budgets(app))
}

/// Existing oversize preferences may survive an environment change; reject new oversize edits.
pub fn issues(app: &AppHandle, next: &Settings, saved: &Settings, text_scale: u32) -> Vec<Issue> {
    let sizes = resolve(app, next, text_scale);
    [DisplayMode::ScreenEdge, DisplayMode::CompactHud].into_iter().filter_map(|mode| {
        let changed = requested(next, mode) != requested(saved, mode)
            || base_size(next, mode) != base_size(saved, mode)
            || next.windows.snap_gap_logical_px != saved.windows.snap_gap_logical_px
            || next.windows.placements.get(mode.key()).and_then(|p|p.monitor.as_ref())
                != saved.windows.placements.get(mode.key()).and_then(|p|p.monitor.as_ref());
        let size = &sizes[mode.key()];
        (changed && size.requested_percent > size.max_percent).then(|| Issue {
            field: format!("widgetAppearance.{}ScalePercent", if mode == DisplayMode::ScreenEdge {"edge"} else {"hud"}),
            message: format!("This layout fits up to {}% on its monitor. Choose a smaller size or another monitor.", size.max_percent),
        })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn monitor_selection_and_environment_fallback_are_independent_per_mode() {
        let mut settings = Settings::default();
        settings.widget_appearance.edge_scale_percent = 200;
        settings.widget_appearance.hud_scale_percent = 80;
        settings
            .windows
            .placements
            .get_mut("screen-edge")
            .unwrap()
            .monitor = Some("large".into());
        let budgets = BTreeMap::from([
            (
                String::new(),
                MonitorBudget {
                    available_width: 300.0,
                    available_height: 300.0,
                },
            ),
            (
                "large".into(),
                MonitorBudget {
                    available_width: 1000.0,
                    available_height: 2000.0,
                },
            ),
        ]);
        let large = resolve_with_budgets(&settings, 100, &budgets);
        assert_eq!(large["screen-edge"].effective_percent, 200);
        assert_eq!(large["compact-hud"].effective_percent, 80);
        let smaller = BTreeMap::from([(
            String::new(),
            MonitorBudget {
                available_width: 300.0,
                available_height: 300.0,
            },
        )]);
        let fallback = resolve_with_budgets(&settings, 100, &smaller);
        assert_eq!(fallback["screen-edge"].effective_percent, 100);
        assert_eq!(settings.widget_appearance.edge_scale_percent, 200);
        assert!(fallback["screen-edge"].fallback_reason.is_some());
        assert_eq!(
            resolve_with_budgets(&settings, 100, &budgets)["screen-edge"].effective_percent,
            200
        );
    }
    #[test]
    fn shared_layouts_keep_reset_bars_and_latest_geometry() {
        let fixtures: serde_json::Value =
            serde_json::from_str(include_str!("../../tests/fixtures/compact-scaling.json"))
                .unwrap();
        for f in fixtures.as_array().unwrap() {
            let mode: DisplayMode = serde_json::from_value(f["mode"].clone()).unwrap();
            let mut settings = Settings::default();
            settings.windows.quota_label_style =
                serde_json::from_value(f["labels"].clone()).unwrap();
            settings.windows.visible_periods.insert(
                mode.key().into(),
                serde_json::from_value(f["periods"].clone()).unwrap(),
            );
            settings.windows.reset_countdowns.insert(
                mode.key().into(),
                serde_json::from_value(f["resets"].clone()).unwrap(),
            );
            let base = base_size(&settings, mode);
            assert_eq!(
                base,
                (f["width"].as_f64().unwrap(), f["height"].as_f64().unwrap())
            );
            for text in [100, 150, 225] {
                for percent in [80, 100, 125, 137, 150, 200, 300] {
                    let factor = f64::from(text) * f64::from(percent) / 10000.0;
                    let available = (base.0 * factor + 16.0, base.1 * factor + 16.0);
                    let state = fit(percent, base, available, text, 8);
                    assert_eq!(state.max_percent, percent);
                    assert_eq!(state.effective_percent, percent);
                    assert!(state.fallback_reason.is_none());
                }
            }
        }
    }
    #[test]
    fn boundaries_accessibility_and_fallback_do_not_change_saved_size() {
        for value in [80, 100, 125, 137, 150, 200, 300] {
            let state = fit(value, (100.0, 40.0), (316.0, 136.0), 100, 8);
            assert_eq!(state.effective_percent, value);
            assert_eq!(state.max_percent, 300);
            assert!(state.fallback_reason.is_none());
        }
        let exact = fit(137, (100.0, 40.0), (153.0, 100.0), 100, 8);
        assert_eq!(exact.max_percent, 137);
        assert_eq!(
            fit(138, (100.0, 40.0), (153.0, 100.0), 100, 8).effective_percent,
            100
        );
        let large_text = fit(200, (100.0, 40.0), (316.0, 136.0), 200, 8);
        assert_eq!(large_text.max_percent, 150);
        assert_eq!(large_text.requested_percent, 200);
        assert_eq!(large_text.effective_percent, 100);
        assert_eq!(
            fit(80, (100.0, 40.0), (20.0, 20.0), 100, 8).effective_percent,
            80
        );
    }
}
