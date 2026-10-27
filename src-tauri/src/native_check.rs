//! Explicit isolated native integration checks (appearance/sizing also in release). Exercises this application's coordinator, never
//! sends OS mouse/keyboard input, and records no quota values or account identifiers.
use crate::{
    interaction::{DetailState, ShellState, Surface},
    model::{
        AppState, DisplayMode, QuotaLabelStyle, ResetCountdowns, Settings, ThemePreference,
        VisiblePeriods,
    },
    service::{Action, Service},
    windows::{Input, Request},
};
use std::time::Duration;
use tauri::{AppHandle, Manager};

static PAINT_POINT: std::sync::Mutex<Option<[f64; 2]>> = std::sync::Mutex::new(None);
static FRONTEND_ASSERT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
pub fn record_frontend_assertion(message: &str) {
    if let Some(point) = message
        .strip_prefix("paint-point:")
        .and_then(|s| serde_json::from_str::<[f64; 2]>(s).ok())
        .filter(|p| {
            p.iter()
                .all(|v| v.is_finite() && (0.0..=10000.0).contains(v))
        })
    {
        if let Ok(mut target) = PAINT_POINT.lock() {
            *target = Some(point);
        }
    }
    if message == "assert-native:pass" {
        FRONTEND_ASSERT.store(1, std::sync::atomic::Ordering::SeqCst);
    } else if message == "assert-native:fail" {
        FRONTEND_ASSERT.store(2, std::sync::atomic::Ordering::SeqCst);
    }
}
async fn frontend_assert(
    app: &AppHandle,
    label: &str,
    expression: &str,
    name: &str,
) -> Result<(), String> {
    FRONTEND_ASSERT.store(0, std::sync::atomic::Ordering::SeqCst);
    let window = app
        .get_webview_window(label)
        .ok_or("assert-webview-missing")?;
    let code=format!("(()=>{{let count=0;const check=()=>{{let ok=false;try{{ok=Boolean({expression});}}catch{{}}if(!ok&&++count<150){{setTimeout(check,25);return;}}if(!ok)window.__TAURI_INTERNALS__.invoke('surface_action',{{request:{{type:'inspect-frontend',message:'assert-detail='+String(window.__styleFailure)}}}});window.__TAURI_INTERNALS__.invoke('surface_action',{{request:{{type:'inspect-frontend',message:'assert-native:'+(ok?'pass':'fail')}}}});}};check();}})()");
    window.eval(&code).map_err(|_| "assert-eval")?;
    for _ in 0..180 {
        match FRONTEND_ASSERT.load(std::sync::atomic::Ordering::SeqCst) {
            1 => return require(true, name),
            2 => return Err(name.into()),
            _ => tokio::time::sleep(Duration::from_millis(25)).await,
        }
    }
    Err(format!("timeout-{name}"))
}
pub fn requested() -> bool {
    appearance_requested()
        || sizing_requested()
        || cfg!(debug_assertions)
            && std::env::args().any(|arg| {
                matches!(
                    arg.as_str(),
                    "--check-native-windows"
                        | "--check-native-stress"
                        | "--check-native-notifications"
                        | "--check-native-topmost"
                )
            })
}

pub fn validation_directory() -> std::path::PathBuf {
    std::env::current_exe()
        .expect("validation executable")
        .parent()
        .expect("executable directory")
        .join("../validation")
        .join(if appearance_requested() {
            "native-appearance"
        } else {
            "native-sizing"
        })
}

static CHECK_LOG: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
fn record(message: String) {
    println!("{message}");
    if let Ok(mut log) = CHECK_LOG.lock() {
        log.push(message);
    }
}

pub fn appearance_requested() -> bool {
    std::env::args().any(|arg| arg == "--check-native-appearance")
}
fn composition_probe_requested() -> bool {
    appearance_requested() && std::env::args().any(|arg| arg == "--probe-native-composition")
}
pub fn presentation_requested() -> bool {
    appearance_requested() || sizing_requested()
}
pub fn fixture_usage() -> crate::model::Usage {
    let now = crate::model::now_ms();
    crate::model::normalize(
        &serde_json::json!({"rateLimits":{"limitId":"codex","primary":{"usedPercent":32,"windowDurationMins":300,"resetsAt":now/1000+8220},"secondary":{"usedPercent":58,"windowDurationMins":10080,"resetsAt":now/1000+390000}},"rateLimitResetCredits":{"availableCount":3}}),
        now,
        &crate::model::Usage::default(),
    )
}

pub fn sizing_requested() -> bool {
    std::env::args().any(|arg| arg == "--check-native-sizing")
}

pub fn topmost_requested() -> bool {
    cfg!(debug_assertions) && std::env::args().any(|arg| arg == "--check-native-topmost")
}

pub fn notifications_requested() -> bool {
    cfg!(debug_assertions) && std::env::args().any(|arg| arg == "--check-native-notifications")
}

pub fn prepare(mut loaded: crate::storage::Loaded) -> crate::storage::Loaded {
    if requested() {
        // This helper is called only after setup chooses the isolated validation directory.
        loaded.settings.reminders = crate::reminders::Preferences::default();
        loaded.reminders = crate::reminders::Durable::default();
    }
    loaded
}

async fn wait(
    app: &AppHandle,
    service: &Service,
    label: &str,
    predicate: impl Fn(&AppState) -> bool,
) -> Result<AppState, String> {
    for _ in 0..160 {
        let state = service.snapshot().await?;
        if predicate(&state) {
            return Ok(state);
        }
        if let Some(error) = &state.window_diagnostic {
            return Err(format!("{label}: {error}"));
        }
        if app.webview_windows().len() > 3 {
            return Err("window-count-exceeded".into());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let final_state = service.snapshot().await?;
    eprintln!("native-check timeout phase={label} mode={:?} generation={} shell={:?} detail={:?} hidden={} suppressed={} windows={} main_visible={} shell_visible={} preview_visible={} diagnostic={}",final_state.settings.mode,final_state.presentation.generation,final_state.presentation.shell,final_state.presentation.detail,final_state.presentation.hidden,final_state.presentation.suppressed,app.webview_windows().len(),visible(app,"main"),visible(app,"shell"),visible(app,"preview"),final_state.window_diagnostic.as_deref().unwrap_or("none"));
    Err(format!("timeout-{label}"))
}

fn visible(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label)
        .is_some_and(|w| w.is_visible().unwrap_or(false))
}
async fn mode(service: &Service, mode: DisplayMode) -> Result<(), String> {
    let state = service.snapshot().await?;
    let mut settings = state.settings;
    settings.mode = mode;
    settings.windows.suppress_fullscreen = false;
    save_settings(service, settings).await
}

async fn save_settings(service: &Service, settings: Settings) -> Result<(), String> {
    let (send, reply) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::Save {
            storage_resolution: None,
            storage_token: None,
            expected_revision: service.snapshot().await?.settings.revision,
            settings: Box::new(settings),
            reply: send,
        })
        .await
        .map_err(|_| "service-stopped")?;
    reply.await.map_err(|_| "service-stopped")??;
    Ok(())
}
fn require(value: bool, label: &str) -> Result<(), String> {
    if value {
        record(format!("native-check {label}=pass"));
        Ok(())
    } else {
        Err(label.into())
    }
}
fn styles(app: &AppHandle, label: &str) -> Option<u32> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWL_EXSTYLE};
    Some(unsafe {
        GetWindowLongPtrW(
            app.get_webview_window(label)?.hwnd().ok()?.0 as _,
            GWL_EXSTYLE,
        ) as u32
    })
}

async fn second_launch(background: bool) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|_| "second-launch-executable")?;
    let mut command = tokio::process::Command::new(exe);
    if background {
        command.arg(crate::lifecycle::LOGIN_ARGUMENT);
    }
    let mut child = command
        .creation_flags(0x08000000)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| "second-launch-spawn")?;
    let status = tokio::time::timeout(Duration::from_secs(15), child.wait())
        .await
        .map_err(|_| "second-launch-timeout")?
        .map_err(|_| "second-launch-wait")?;
    require(
        status.success(),
        if background {
            "duplicate-login-exits"
        } else {
            "duplicate-manual-exits"
        },
    )
}

pub fn start(app: AppHandle, service: Service) {
    if presentation_requested() {
        let directory = validation_directory();
        let _ = std::fs::create_dir_all(&directory);
        let _ = std::fs::write(directory.join("result.log"), "native-check result=running");
    }
    tauri::async_runtime::spawn(async move {
        let mut result = if appearance_requested() {
            appearance_check(&app, &service).await
        } else if topmost_requested() {
            reveal_check(&app, &service).await
        } else if sizing_requested()
            && std::env::args().any(|arg| arg == "--probe-native-activation")
        {
            service.dispatch(Action::Window(Input::Close));
            tokio::time::sleep(Duration::from_millis(200)).await;
            async {
                dashboard_navigation_check(&app, &service).await?;
                settings_preview_check(&app, &service).await?;
                floating_close_to_tray(&app, &service).await?;
                activation_fallback(&app, &service).await
            }
            .await
        } else if sizing_requested() {
            compact_sizing(&app, &service).await
        } else {
            run(&app, &service).await
        };
        if cfg!(debug_assertions)
            && result.is_ok()
            && std::env::args().any(|arg| arg == "--check-native-stress")
        {
            result = stress(&app, &service).await;
        }
        if result.is_ok() && notifications_requested() {
            result = notification_check(&app, &service).await;
        }
        match result {
            Ok(()) => record("native-check result=pass".into()),
            Err(error) => record(format!("native-check result=failed code={error}")),
        }
        if presentation_requested() {
            let directory = validation_directory();
            let _ = std::fs::create_dir_all(&directory);
            if let Ok(log) = CHECK_LOG.lock() {
                let _ = std::fs::write(directory.join("result.log"), log.join("\n"));
            }
        }
        service.dispatch(Action::Quit);
    });
}

// Independent of foreground activation: compare React's generation-matched
// content measurement with the actual WebView client area, not outer bounds.
async fn compact_sizing(app: &AppHandle, service: &Service) -> Result<(), String> {
    let original = service.snapshot().await?.settings;
    service.dispatch(Action::Window(Input::Close));
    for theme in [ThemePreference::Light, ThemePreference::Dark] {
        for (five_hour, weekly) in [(false, false), (true, false), (false, true), (true, true)] {
            let countdowns = five_hour && weekly;
            for periods in [
                VisiblePeriods::Both,
                VisiblePeriods::FiveHour,
                VisiblePeriods::Weekly,
            ] {
                for (mode, label_style) in [
                    (DisplayMode::Floating, QuotaLabelStyle::Words),
                    (DisplayMode::ScreenEdge, QuotaLabelStyle::Words),
                    (DisplayMode::ScreenEdge, QuotaLabelStyle::Badges),
                    (DisplayMode::CompactHud, QuotaLabelStyle::Words),
                    (DisplayMode::CompactHud, QuotaLabelStyle::Badges),
                ] {
                    if mode == DisplayMode::Floating && periods != VisiblePeriods::Both {
                        continue;
                    }
                    let mut settings = service.snapshot().await?.settings;
                    if mode != DisplayMode::Floating {
                        settings
                            .windows
                            .visible_periods
                            .insert(mode.key().into(), periods);
                    }
                    settings.mode = mode;
                    settings.theme_preference = theme;
                    settings.windows.hover_enabled = true;
                    settings.windows.quota_label_style = label_style;
                    settings.windows.always_on_top = countdowns;
                    settings.windows.suppress_fullscreen = false;
                    settings
                        .windows
                        .reset_countdowns
                        .insert(mode.key().into(), ResetCountdowns { five_hour, weekly });
                    save_settings(service, settings).await?;
                    wait(app, service, "sizing-hydration", |s| {
                        s.settings.mode == mode && visible(app, "shell")
                    })
                    .await?;
                    let shell = app
                        .get_webview_window("shell")
                        .ok_or("sizing-shell-missing")?;
                    let text_scale = service.snapshot().await?.appearance.text_scale as f64 / 100.0;
                    let count = u8::from(five_hour && periods.includes(300))
                        + u8::from(weekly && periods.includes(10080));
                    let single = periods != VisiblePeriods::Both;
                    let expected = match mode {
                        DisplayMode::ScreenEdge => Some((
                            54.0,
                            (if label_style == QuotaLabelStyle::Words {
                                if single {
                                    116.0
                                } else {
                                    186.0
                                }
                            } else {
                                if single {
                                    96.0
                                } else {
                                    146.0
                                }
                            }) + 24.0 * f64::from(count),
                        )),
                        DisplayMode::CompactHud => Some((
                            (if label_style == QuotaLabelStyle::Badges {
                                if single {
                                    102.0
                                } else {
                                    164.0
                                }
                            } else {
                                if single {
                                    120.0
                                } else {
                                    200.0
                                }
                            }) + 4.0
                                + 58.0 * f64::from(count),
                            38.0,
                        )),
                        _ => None,
                    };
                    let mut stable = 0;
                    let mut last_size = None;
                    for _ in 0..100 {
                        let sizing_snapshot = service.snapshot().await?;
                        let u = &sizing_snapshot.usage;
                        let warning = u.freshness == "stale"
                            || u.windows
                                .iter()
                                .filter(|w| {
                                    mode == DisplayMode::Floating
                                        || w.duration_minutes
                                            .is_some_and(|duration| periods.includes(duration))
                                })
                                .any(|w| {
                                    w.last_known
                                        || w.quality != "valid"
                                        || w.remaining_percent.is_none()
                                })
                            || u.ordinary_usage_allowed == Some(false)
                            || !u
                                .windows
                                .iter()
                                .filter(|w| {
                                    mode == DisplayMode::Floating
                                        || w.duration_minutes
                                            .is_some_and(|duration| periods.includes(duration))
                                })
                                .any(|w| w.remaining_percent.is_some());
                        let desired = expected.map(|(w, h)| {
                            let extra = if warning {
                                if mode == DisplayMode::ScreenEdge {
                                    18.0
                                } else {
                                    20.0
                                }
                            } else {
                                0.0
                            };
                            (
                                (w + if mode == DisplayMode::CompactHud {
                                    extra
                                } else {
                                    0.0
                                }) * text_scale,
                                (h + if mode == DisplayMode::ScreenEdge {
                                    extra
                                } else {
                                    0.0
                                }) * text_scale,
                            )
                        });
                        let (reply, receive) = tokio::sync::oneshot::channel();
                        service
                            .0
                            .send(Action::ShellContentSize(reply))
                            .await
                            .map_err(|_| "service-stopped")?;
                        let content = receive.await.map_err(|_| "service-stopped")?;
                        let client = shell.inner_size().map_err(|_| "sizing-client")?;
                        let scale = shell.scale_factor().map_err(|_| "sizing-scale")?;
                        let size = content
                            .map(|c| ((c.0 * scale).ceil() as u32, (c.1 * scale).ceil() as u32));
                        if size == Some((client.width, client.height))
                            && size == last_size
                            && desired.is_none_or(|(w, h)| {
                                content.is_some_and(|(cw, ch)| {
                                    (cw - w.ceil()).abs() <= 1.0 && (ch - h.ceil()).abs() <= 1.0
                                })
                            })
                        {
                            stable += 1;
                            if stable == 3 {
                                break;
                            }
                        } else {
                            stable = 0;
                        }
                        last_size = size;
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    if stable != 3 {
                        return Err(format!(
                        "sizing-client-mismatch mode={} theme={theme:?} labels={label_style:?} periods={periods:?} five-hour={five_hour} weekly={weekly}",
                        mode.key()
                    ));
                    }
                    let outer = shell.outer_size().map_err(|_| "sizing-outer")?;
                    let client = shell.inner_size().map_err(|_| "sizing-client")?;
                    let scale = shell.scale_factor().map_err(|_| "sizing-scale")?;
                    println!("native-check sizing mode={} theme={theme:?} labels={label_style:?} five-hour={five_hour} weekly={weekly} scale={scale} frame={}x{} client-fits=pass", mode.key(), outer.width.saturating_sub(client.width), outer.height.saturating_sub(client.height));
                    require(
                        crate::native_windows::has_rounded_corners(&shell),
                        "shell-rounded-region",
                    )?;
                    if mode == DisplayMode::Floating {
                        let state = service.snapshot().await?;
                        service.dispatch(Action::Window(Input::Pointer {
                            generation: state.presentation.generation,
                            source: Surface::Shell,
                            trigger: true,
                            preview: false,
                            corridor: false,
                        }));
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        require(
                            service.snapshot().await?.presentation.shell == ShellState::Compact
                                && !visible(app, "preview"),
                            "floating-has-no-hover",
                        )?;
                    } else {
                        let foreground = unsafe {
                            windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
                        } as usize;
                        open_hover(app, service, Surface::Shell).await?;
                        preview_sizing(app, service, Surface::Shell).await?;
                        let preview = app.get_webview_window("preview").ok_or("preview-missing")?;
                        require(
                            crate::native_windows::is_above(&preview, &shell),
                            "preview-above-shell",
                        )?;
                        let after = unsafe {
                            windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
                        } as usize;
                        if after != foreground {
                            let owned = |window: usize| {
                                let mut process = 0;
                                unsafe {
                                    windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(window as _,&mut process);
                                };
                                process == std::process::id()
                            };
                            println!(
                                "native-check foreground-change before-owned={} after-owned={}",
                                owned(foreground),
                                owned(after)
                            );
                        }
                        require(after == foreground, "hover-preserves-foreground")?;
                        service.dispatch(Action::Window(Input::Request(
                            Surface::Preview,
                            Request::Collapse,
                        )));
                        wait(app, service, "sizing-preview-hidden", |s| {
                            s.presentation.shell == ShellState::Compact && !visible(app, "preview")
                        })
                        .await?;
                    }
                }
            }
        }
        if let Some(rect) = app.tray_by_id("main").and_then(|t| t.rect().ok().flatten()) {
            let p = rect.position.to_physical::<i32>(1.0);
            let s = rect.size.to_physical::<u32>(1.0);
            service.dispatch(Action::Window(Input::Tray {
                rect: crate::geometry::Rect {
                    x: p.x as f64,
                    y: p.y as f64,
                    width: s.width as f64,
                    height: s.height as f64,
                },
                activate: false,
                open_window: false,
                menu: false,
            }));
            open_hover(app, service, Surface::Tray).await?;
            preview_sizing(app, service, Surface::Tray).await?;
            service.dispatch(Action::Window(Input::Request(
                Surface::Preview,
                Request::Collapse,
            )));
        } else {
            println!("native-check tray-preview-sizing=unverified-no-tray-rectangle");
        }
    }
    edge_docking(app, service).await?;
    appearance_check(app, service).await?;
    reveal_check(app, service).await?;
    popup_stacking(app, service).await?;
    floating_close_to_tray(app, service).await?;
    activation_fallback(app, service).await?;
    save_settings(service, original).await?;
    require(true, "compact-client-sizing")
}

/// Synthetic native paint/placement check. No Codex process or OS input is used.
async fn appearance_check(app: &AppHandle, service: &Service) -> Result<(), String> {
    use crate::geometry::{HorizontalAnchor, VerticalAnchor};
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let original = service.snapshot().await?.settings;
    let foreground = unsafe { GetForegroundWindow() } as usize;
    service.dispatch(Action::Window(Input::Close));
    for mode in [
        DisplayMode::Floating,
        DisplayMode::ScreenEdge,
        DisplayMode::CompactHud,
    ] {
        for gap in [0, 8, 16, 48] {
            let mut settings = service.snapshot().await?.settings;
            settings.mode = mode;
            settings.windows.snap_gap_logical_px = gap;
            settings.windows.lock_position = true;
            settings.windows.always_on_top = true;
            settings.windows.suppress_fullscreen = false;
            settings.widget_appearance.background_transparency_percent = 0;
            save_settings(service, settings).await?;
            let revision = service.snapshot().await?.settings.revision;
            service.dispatch(Action::Window(Input::Request(
                Surface::Tray,
                Request::ResetPosition,
            )));
            wait(app, service, "gap-reset", |s| {
                s.settings.revision > revision && visible(app, "shell")
            })
            .await?;
            let shell = app.get_webview_window("shell").ok_or("gap-shell")?;
            let monitor = shell
                .current_monitor()
                .map_err(|_| "gap-monitor")?
                .ok_or("gap-monitor")?;
            let work = monitor.work_area();
            let mut matched = false;
            for _ in 0..100 {
                let state = service.snapshot().await?;
                let p = shell.outer_position().map_err(|_| "gap-position")?;
                let size = shell.outer_size().map_err(|_| "gap-size")?;
                let m = state.appearance.placement_constraints.margins;
                let x = if mode == DisplayMode::ScreenEdge
                    && state.settings.windows.edge_side == crate::model::EdgeSide::Right
                {
                    f64::from(work.position.x) + f64::from(work.size.width)
                        - f64::from(size.width)
                        - m[2]
                } else {
                    f64::from(work.position.x) + m[0]
                };
                if (f64::from(p.x) - x).abs() <= 1.0
                    && (f64::from(p.y) - f64::from(work.position.y) - m[1]).abs() <= 1.0
                {
                    matched = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(30)).await;
            }
            require(matched, "shared-gap-visible-bounds")?;
            let state = service.snapshot().await?;
            let anchors = &state.settings.windows.snap_anchors[mode.key()];
            require(
                anchors.vertical == VerticalAnchor::Top
                    && anchors.horizontal
                        == if mode == DisplayMode::ScreenEdge {
                            HorizontalAnchor::None
                        } else {
                            HorizontalAnchor::Left
                        },
                "reset-persists-anchor-intent",
            )?;
            require(
                crate::native_windows::has_rounded_corners(&shell),
                "transparent-shell-retains-rounded-region",
            )?;
        }
        for percent in [0, 20, 50, 80] {
            let mut settings = service.snapshot().await?.settings;
            settings.widget_appearance.background_transparency_percent = percent;
            save_settings(service, settings).await?;
            let shell = app.get_webview_window("shell").ok_or("paint-shell")?;
            let snapshot = service.snapshot().await?;
            println!(
                "native-check requested-alpha={percent} effective={}",
                snapshot.appearance.widget.background_alpha
            );
            shell.eval("window.__TAURI_INTERNALS__.invoke('surface_action',{request:{type:'inspect-frontend',message:'paint='+getComputedStyle(document.querySelector('.shell')).backgroundImage}})").map_err(|_|"paint-style")?;
            require(
                snapshot.appearance.widget.background_alpha == 1.0
                    && snapshot.appearance.widget.backing_alpha == 1.0
                    && snapshot
                        .settings
                        .widget_appearance
                        .background_transparency_percent
                        == percent,
                "saved-transparency-is-inactive",
            )?;
            frontend_assert(
                app,
                "shell",
                "(()=>{const s=document.querySelector('.shell');return s&&getComputedStyle(s).opacity==='1'&&!s.querySelector('.surface-background')&&!s.hasAttribute('data-surface-paint')&&[...s.querySelectorAll('.quota-reading strong,.ring-reading strong,.hud-quota>strong,.ring-value,.progress>span,.hud-track>i,.reset-segments i')].every(e=>getComputedStyle(e).opacity==='1');})()",
                "opaque-background-with-opaque-foreground",
            )
            .await?;
            if !composition_probe_requested() {
                continue;
            }
            let black = composition_probe(app, &shell, false, None).await?;
            let white = composition_probe(app, &shell, true, None).await?;
            if percent == 0 {
                let background = &snapshot.appearance.widget.palette["background"];
                require(
                    (0..3).all(|channel| {
                        black[channel].abs_diff(
                            u8::from_str_radix(&background[1 + channel * 2..3 + channel * 2], 16)
                                .unwrap(),
                        ) <= 24
                    }),
                    "opaque-capture-calibrates-to-rendered-theme",
                )?;
            }
            let expected = 0.0;
            record(format!(
                "native-check composition requested={percent} delta={:?}",
                (0..3)
                    .map(|i| i16::from(white[i]) - i16::from(black[i]))
                    .collect::<Vec<_>>()
            ));
            require(
                (0..3).all(|channel| {
                    (f64::from(white[channel]) - f64::from(black[channel]) - expected).abs() <= 10.0
                }),
                "background-only-native-composition",
            )?;
            if mode == DisplayMode::Floating {
                for selector in [".quota-card", ".widget-credits"] {
                    *PAINT_POINT.lock().map_err(|_| "paint-point-lock")? = None;
                    let expression=format!("(()=>{{const e=document.querySelector('.shell {selector}');if(!e)return false;const r=e.getBoundingClientRect();window.__TAURI_INTERNALS__.invoke('surface_action',{{request:{{type:'inspect-frontend',message:'paint-point:'+JSON.stringify([r.left+4,r.top+r.height/2])}}}});return true;}})()");
                    frontend_assert(app, "shell", &expression, "inner-surface-paint-point").await?;
                    let mut sample = None;
                    for _ in 0..50 {
                        sample = *PAINT_POINT.lock().map_err(|_| "paint-point-lock")?;
                        if sample.is_some() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    let sample = sample.ok_or("paint-point-unavailable")?;
                    let black = composition_probe(app, &shell, false, Some(sample)).await?;
                    let white = composition_probe(app, &shell, true, Some(sample)).await?;
                    let delta = (0..3)
                        .map(|i| i16::from(white[i]) - i16::from(black[i]))
                        .collect::<Vec<_>>();
                    record(format!("native-check inner-composition surface={selector} requested={percent} delta={delta:?}"));
                    require(
                        (0..3).all(|i| {
                            (f64::from(white[i]) - f64::from(black[i]) - expected).abs() <= 10.0
                        }),
                        "inner-surface-native-composition",
                    )?;
                }
            }
        }
    }
    require(
        unsafe { GetForegroundWindow() } as usize == foreground,
        "appearance-preserves-foreground",
    )?;
    require(
        !service.snapshot().await?.usage.refresh_in_flight,
        "synthetic-check-has-no-quota-read",
    )?;
    original_style_check(app, service).await?;
    showcase_style_check(app, service).await?;
    dashboard_navigation_check(app, service).await?;
    settings_preview_check(app, service).await?;
    save_settings(service, original).await?;
    Ok(())
}

async fn showcase_style_check(app: &AppHandle, service: &Service) -> Result<(), String> {
    use crate::widget_appearance::{PresetId, WidgetAppearance};
    let catalog: serde_json::Value =
        serde_json::from_str(include_str!("../../src/appearance-presets.json")).unwrap();
    service.dispatch(Action::Window(Input::Close));
    for dark in [false, true] {
        for id in catalog.as_object().unwrap().keys() {
            let preset: PresetId = serde_json::from_value(id.clone().into()).unwrap();
            for mode in [
                DisplayMode::Floating,
                DisplayMode::ScreenEdge,
                DisplayMode::CompactHud,
            ] {
                let mut settings = service.snapshot().await?.settings;
                settings.mode = mode;
                settings
                    .windows
                    .visible_periods
                    .insert("screen-edge".into(), VisiblePeriods::Both);
                settings
                    .windows
                    .visible_periods
                    .insert("compact-hud".into(), VisiblePeriods::Both);
                settings.theme_preference = if dark {
                    ThemePreference::Dark
                } else {
                    ThemePreference::Light
                };
                settings.widget_appearance = WidgetAppearance {
                    preset_id: Some(preset),
                    ..Default::default()
                };
                save_settings(service, settings).await?;
                let palette =
                    serde_json::to_string(&crate::widget_appearance::preset_palette(preset, dark))
                        .unwrap();
                let (five, weekly) = match mode {
                    DisplayMode::Floating => (
                        ".period-five-hour .quota-reading strong",
                        ".period-weekly .quota-reading strong",
                    ),
                    DisplayMode::ScreenEdge => (
                        ".edge-values>.edge-value:not(.weekly) .ring-reading>strong",
                        ".weekly .ring-reading>strong",
                    ),
                    _ => (".hud-quota:not(.weekly)>strong", ".hud-quota.weekly>strong"),
                };
                let theme = if dark { "dark" } else { "light" };
                let expression=format!("(()=>{{if(document.documentElement.dataset.theme!=='{theme}')return false;const s=document.querySelector('.shell'),p={palette},rgb=h=>'rgb('+[1,3,5].map(i=>parseInt(h.slice(i,i+2),16)).join(', ')+')';if(!s)return false;const f=s.querySelector('{five}'),w=s.querySelector('{weekly}');return f&&w&&getComputedStyle(f).color===rgb(p.fiveHour)&&getComputedStyle(w).color===rgb(p.weekly)&&getComputedStyle(s).backgroundImage.includes(rgb(p.background))&&getComputedStyle(s).borderTopColor===(s.style.getPropertyValue('--widget-frame')==='transparent'?'rgba(0, 0, 0, 0)':rgb(s.style.getPropertyValue('--widget-frame')))&&[...s.querySelectorAll('.edge-value+.edge-value,.hud-quota+.hud-quota')].every(e=>{{const c=getComputedStyle(e);return (s.classList.contains('mode-screen-edge')?c.borderTopColor:c.borderLeftColor)===rgb(s.style.getPropertyValue('--widget-divider'));}});}})()");
                frontend_assert(
                    app,
                    "shell",
                    &expression,
                    &format!("showcase-{id}-{theme}-{mode:?}"),
                )
                .await?;
                let shell = app.get_webview_window("shell").ok_or("border-shell")?;
                let size = shell.inner_size().map_err(|_| "border-size")?;
                for show in [false, true] {
                    let mut settings = service.snapshot().await?.settings;
                    settings.widget_appearance.show_border = show;
                    save_settings(service, settings).await?;
                    let frame = serde_json::to_string(
                        &service.snapshot().await?.appearance.widget.frame_border,
                    )
                    .unwrap();
                    let expression = format!("(()=>{{const s=document.querySelector('.native-surface'),expected={frame},rgb=h=>'rgb('+[1,3,5].map(i=>parseInt(h.slice(i,i+2),16)).join(', ')+')';return s&&getComputedStyle(s).borderTopWidth==='1px'&&getComputedStyle(s).borderTopColor===(expected==='transparent'?'rgba(0, 0, 0, 0)':rgb(expected));}})()");
                    frontend_assert(
                        app,
                        "shell",
                        &expression,
                        &format!("border-{id}-{theme}-{mode:?}-{show}"),
                    )
                    .await?;
                    require(
                        shell.inner_size().map_err(|_| "border-size")? == size,
                        "border-keeps-widget-dimensions",
                    )?;
                    if !show && id == "original" && !dark {
                        frontend_assert(app, "shell", "(()=>{const r=document.documentElement,s=document.querySelector('.shell'),previous=r.dataset.highContrast;try{r.dataset.highContrast='true';return s&&getComputedStyle(s).borderTopColor!=='rgba(0, 0, 0, 0)'&&getComputedStyle(s).borderTopWidth==='1px';}finally{r.dataset.highContrast=previous;}})()", "synthetic-high-contrast-retains-border").await?;
                    }
                    if mode != DisplayMode::Floating {
                        open_hover(app, service, Surface::Shell).await?;
                        frontend_assert(
                            app,
                            "preview",
                            &expression,
                            "desktop-preview-shares-border-preference",
                        )
                        .await?;
                        service.dispatch(Action::Window(Input::Request(
                            Surface::Preview,
                            Request::Collapse,
                        )));
                    }
                }
            }
            for mode in [
                DisplayMode::Floating,
                DisplayMode::ScreenEdge,
                DisplayMode::CompactHud,
            ] {
                let mut settings = service.snapshot().await?.settings;
                settings.mode = mode;
                let pair = settings
                    .widget_appearance
                    .quota_customizations
                    .entry(preset)
                    .or_default();
                let c = if dark {
                    &mut pair.dark
                } else {
                    &mut pair.light
                };
                c.enabled = true;
                c.five_hour = Some("#FACC15".into());
                c.weekly = Some("#F97316".into());
                let expected = serde_json::to_string(
                    &settings.widget_appearance.resolve(dark, false, true, true),
                )
                .unwrap();
                save_settings(service, settings).await?;
                let expression=format!("(()=>{{const s=document.querySelector('.shell'),p={expected},rgb=h=>'rgb('+[1,3,5].map(i=>parseInt(h.slice(i,i+2),16)).join(', ')+')';if(!s)return false;for(const [period,role]of [['five-hour','fiveHour'],['weekly','weekly']]){{const reading=s.querySelector('.period-'+period+' .quota-reading strong')??s.querySelector(role==='weekly'?'.weekly .ring-reading strong,.hud-quota.weekly>strong':'.edge-value:not(.weekly) .ring-reading strong,.hud-quota:not(.weekly)>strong');const fill=s.querySelector('.period-'+period+' .progress>span')??s.querySelector(role==='weekly'?'.weekly .ring-value,.weekly .hud-track>i':'.edge-value:not(.weekly) .ring-value,.hud-quota:not(.weekly) .hud-track>i');if(!reading||!fill||getComputedStyle(reading).color!==rgb(p.quotaText[role])||(fill.classList.contains('ring-value')?getComputedStyle(fill).stroke:getComputedStyle(fill).backgroundColor)!==rgb(p.palette[role]))return false;}}return true;}})()");
                frontend_assert(
                    app,
                    "shell",
                    &expression,
                    &format!(
                        "bright-quota-{id}-{}-{mode:?}",
                        if dark { "dark" } else { "light" }
                    ),
                )
                .await?;
            }
            // Use this session's own shell as a synthetic tray anchor. This
            // checks tray rendering without requiring real pointer input.
            let shell = app.get_webview_window("shell").ok_or("tray-style-anchor")?;
            let position = shell.outer_position().map_err(|_| "tray-style-position")?;
            let size = shell.outer_size().map_err(|_| "tray-style-size")?;
            let mut settings = service.snapshot().await?.settings;
            settings.mode = DisplayMode::SystemTray;
            save_settings(service, settings).await?;
            service.dispatch(Action::Window(Input::Tray {
                rect: crate::geometry::Rect {
                    x: f64::from(position.x),
                    y: f64::from(position.y),
                    width: f64::from(size.width),
                    height: f64::from(size.height),
                },
                activate: false,
                open_window: false,
                menu: false,
            }));
            open_hover(app, service, Surface::Tray).await?;
            let palette = crate::widget_appearance::preset_palette(preset, dark);
            let background = if preset == PresetId::Original {
                if dark {
                    "#10213A"
                } else {
                    "#FFFFFF"
                }
            } else {
                &palette["contentSurface"]
            };
            let text = &palette["text"];
            let expression=format!("(()=>{{const s=document.querySelector('.tray-summary'),rgb=h=>'rgb('+[1,3,5].map(i=>parseInt(h.slice(i,i+2),16)).join(', ')+')';return s&&!s.querySelector('button')&&getComputedStyle(s).backgroundColor===rgb('{background}')&&getComputedStyle(s).color===rgb('{text}');}})()");
            frontend_assert(
                app,
                "preview",
                &expression,
                &format!(
                    "showcase-{id}-{}-TrayPreview",
                    if dark { "dark" } else { "light" }
                ),
            )
            .await?;
            for show in [false, true] {
                let mut settings = service.snapshot().await?.settings;
                settings.widget_appearance.show_border = show;
                save_settings(service, settings).await?;
                open_hover(app, service, Surface::Tray).await?;
                let expression = format!("(()=>{{const s=document.querySelector('.tray-summary');return s&&(getComputedStyle(s).borderTopColor==='rgba(0, 0, 0, 0)')===!{show}&&getComputedStyle(s).borderTopWidth==='1px';}})()");
                frontend_assert(
                    app,
                    "preview",
                    &expression,
                    "tray-preview-shares-border-preference",
                )
                .await?;
            }
            service.dispatch(Action::Window(Input::Request(
                Surface::Preview,
                Request::Collapse,
            )));
        }
    }
    Ok(())
}

async fn dashboard_content_fit(
    app: &AppHandle,
    main: &tauri::WebviewWindow,
    cap: f64,
    name: &str,
) -> Result<(), String> {
    let outer = main.outer_size().map_err(|_| "fit-outer")?;
    let inner = main.inner_size().map_err(|_| "fit-inner")?;
    let scale = main.scale_factor().map_err(|_| "fit-scale")?;
    let limit = (cap - f64::from(outer.height.saturating_sub(inner.height))) / scale;
    let expression=format!("(()=>{{const body=document.querySelector('.content-body'),viewport=document.querySelector('.content'),frame=document.querySelector('.app-frame');if(!body||!viewport||!frame)return false;const s=getComputedStyle(viewport),chrome=[...frame.children].filter(e=>e.matches('.app-header,.tabs,.app-footer')).reduce((v,e)=>v+e.getBoundingClientRect().height,0),natural=Math.ceil(body.getBoundingClientRect().height+parseFloat(s.paddingTop)+parseFloat(s.paddingBottom)+chrome);window.__styleFailure='fit actual='+innerHeight+' natural='+natural+' limit='+{limit};return Math.abs(innerHeight-Math.min(natural,{limit}))<=1;}})()");
    frontend_assert(app, "main", &expression, name).await
}

async fn dashboard_navigation_check(app: &AppHandle, service: &Service) -> Result<(), String> {
    service.dispatch(Action::Window(Input::Close));
    service.dispatch(Action::Window(Input::InspectPopup));
    dashboard_settled(app, service).await?;
    let main = app.get_webview_window("main").ok_or("sizing-main")?;
    let popup_size = main.outer_size().map_err(|_| "sizing-popup")?;
    let monitor = main
        .current_monitor()
        .map_err(|_| "sizing-monitor")?
        .ok_or("sizing-monitor")?;
    let cap = (f64::from(monitor.work_area().size.height) * 0.85).floor();
    require(
        f64::from(popup_size.height) <= cap,
        "popup-natural-height-capped",
    )?;
    for height in [300.0, 20000.0, 400.0] {
        service.dispatch(Action::Window(Input::DashboardLayout {
            generation: service.snapshot().await?.presentation.generation,
            page: crate::model::DashboardPage::Overview,
            width: 480.0,
            height,
            fit: true,
        }));
        tokio::time::sleep(Duration::from_millis(150)).await;
        let size = main.outer_size().map_err(|_| "sizing-content")?;
        let natural = crate::native_windows::content_outer_size(
            &main,
            (480.0, height),
            monitor.scale_factor(),
        )?;
        record(format!("native-check popup-height logical={height} scale={} inner={:?} outer={size:?} expected={} cap={cap}",monitor.scale_factor(),main.inner_size(),natural.1.min(cap)));
        require(
            (f64::from(size.height) - natural.1.min(cap)).abs() <= 1.0,
            "popup-content-height-fits-up-to-cap",
        )?;
    }
    // Restore React's real natural measurement before checking navigation.
    main.eval(
        "document.querySelector('nav[aria-label=Dashboard] button[aria-label=Settings]').click()",
    )
    .map_err(|_| "sizing-settings")?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    main.eval(
        "document.querySelector('nav[aria-label=Dashboard] button[aria-label=Overview]').click()",
    )
    .map_err(|_| "sizing-overview")?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    dashboard_content_fit(app, &main, cap, "popup-natural-overview-content-fits").await?;
    let popup_size = main.outer_size().map_err(|_| "sizing-popup")?;
    for section in [
        "Display",
        "Reminders",
        "General",
        "Connection & diagnostics",
    ] {
        main.eval("document.querySelector('nav[aria-label=Dashboard] button[aria-label=Settings]').click()").map_err(|_|"sizing-settings")?;
        main.eval(format!("[...document.querySelectorAll('.settings-tabs button')].find(b=>b.textContent==='{section}').click()")).map_err(|_|"sizing-section")?;
        frontend_assert(
            app,
            "main",
            "!!document.querySelector('[data-settings]:not([hidden])')",
            "settings-page-visible",
        )
        .await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        require(
            main.outer_size().map_err(|_| "sizing-popup")? == popup_size,
            "popup-navigation-preserves-bounds",
        )?;
        main.eval("document.querySelector('nav[aria-label=Dashboard] button[aria-label=Overview]').click()").map_err(|_|"sizing-overview")?;
        tokio::time::sleep(Duration::from_millis(100)).await;
        require(
            main.outer_size().map_err(|_| "sizing-popup")? == popup_size,
            "popup-overview-preserves-bounds",
        )?;
    }
    service.dispatch(Action::Window(Input::Request(
        Surface::Main,
        Request::Promote,
    )));
    dashboard_settled(app, service).await?;
    let before_move = main.outer_position().map_err(|_| "sizing-move")?;
    main.set_position(tauri::PhysicalPosition::new(
        before_move.x + 10,
        before_move.y + 10,
    ))
    .map_err(|_| "sizing-move")?;
    service.dispatch(Action::Window(Input::Layout));
    tokio::time::sleep(Duration::from_millis(100)).await;
    for height in [300.0, 20000.0, 400.0] {
        service.dispatch(Action::Window(Input::DashboardLayout {
            generation: service.snapshot().await?.presentation.generation,
            page: crate::model::DashboardPage::Overview,
            width: 620.0,
            height,
            fit: true,
        }));
        tokio::time::sleep(Duration::from_millis(150)).await;
        let natural = crate::native_windows::content_outer_size(
            &main,
            (620.0, height),
            monitor.scale_factor(),
        )?;
        require(
            (f64::from(main.outer_size().map_err(|_| "sizing-content")?.height)
                - natural.1.min(cap))
            .abs()
                <= 1.0,
            "standard-content-adapts-after-move-up-to-cap",
        )?;
    }
    main.set_size(tauri::LogicalSize::new(700.0, 520.0))
        .map_err(|_| "sizing-manual")?;
    service.dispatch(Action::Window(Input::MainUserSizing));
    tokio::time::sleep(Duration::from_millis(150)).await;
    let window_size = main.outer_size().map_err(|_| "sizing-window")?;
    main.eval(
        "document.querySelector('nav[aria-label=Dashboard] button[aria-label=Settings]').click()",
    )
    .map_err(|_| "sizing-settings")?;
    tokio::time::sleep(Duration::from_millis(150)).await;
    require(
        main.outer_size().map_err(|_| "sizing-window")? == window_size,
        "standard-navigation-preserves-manual-bounds",
    )?;
    service.dispatch(Action::Window(Input::Close));
    service.dispatch(Action::Window(Input::InspectPopup));
    dashboard_settled(app, service).await?;
    record(format!(
        "native-check popup-reopen expected={popup_size:?} actual={:?} client={:?}",
        main.outer_size(),
        main.inner_size()
    ));
    require(
        main.outer_size().map_err(|_| "sizing-reopen")?.width == popup_size.width,
        "popup-reopen-keeps-independent-width",
    )?;
    dashboard_content_fit(
        app,
        &main,
        cap,
        "popup-reopen-fits-own-content-independently-of-manual-standard-height",
    )
    .await?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Main,
        Request::Promote,
    )));
    dashboard_settled(app, service).await?;
    require(
        main.outer_size().map_err(|_| "sizing-reopen")? == window_size,
        "manual-standard-height-survives-popup-and-reopen",
    )?;
    service.dispatch(Action::Window(Input::Close));
    Ok(())
}

async fn original_style_check(app: &AppHandle, service: &Service) -> Result<(), String> {
    use crate::widget_appearance::{PresetId, WidgetAppearance};
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/original-v0.1.0-colors.json"
    ))
    .unwrap();
    for dark in [false, true] {
        for mode in [
            DisplayMode::Floating,
            DisplayMode::ScreenEdge,
            DisplayMode::CompactHud,
        ] {
            let mut settings = service.snapshot().await?.settings;
            settings.theme_preference = if dark {
                ThemePreference::Dark
            } else {
                ThemePreference::Light
            };
            settings.mode = mode;
            settings.windows.quota_label_style = QuotaLabelStyle::Badges;
            settings.widget_appearance = WidgetAppearance {
                preset_id: Some(PresetId::Original),
                ..Default::default()
            };
            save_settings(service, settings).await?;
            wait(app, service, "original-shell", |_| visible(app, "shell")).await?;
            let theme = if dark { "dark" } else { "light" };
            let expected = serde_json::to_string(&reference[theme]).unwrap();
            let script=format!("(()=>{{const root=getComputedStyle(document.documentElement),s=document.querySelector('.shell');if(!s||s.classList.contains('widget-palette')||document.documentElement.dataset.theme!=='{theme}')return false;const ref={expected};const rgb=h=>'rgb('+[1,3,5].map(i=>parseInt(h.slice(i,i+2),16)).join(', ')+')';const style=getComputedStyle(s);if(Math.abs(parseFloat(style.borderTopWidth)-Math.max(1,Math.floor(devicePixelRatio))/devicePixelRatio)>.02||style.borderTopColor!==rgb(s.style.getPropertyValue('--widget-frame'))){{window.__styleFailure='border='+style.borderTopWidth+' '+style.borderTopColor;return false;}}if(!Object.entries(ref).every(([k,v])=>{{let c=root.getPropertyValue(k).trim().toUpperCase();if(c.length===4)c='#'+[...c.slice(1)].map(x=>x+x).join('');return c===v;}}))return false;const start=s.classList.contains('mode-floating')?'--wash':'--raised';if(!style.backgroundImage.includes(rgb(ref[start]))||!style.backgroundImage.includes(rgb(ref['--shell'])))return false;for(const [q,token]of [['.widget-credits','--wash'],['.compact-card','--raised'],['.period-badge','--wash']]){{for(const el of s.querySelectorAll(q))if(getComputedStyle(el).backgroundColor!==rgb(ref[token]))return false;}}return true;}})()");
            frontend_assert(
                app,
                "shell",
                &script,
                &format!("original-{theme}-{mode:?}-v010-cascade"),
            )
            .await?;
            if mode != DisplayMode::Floating {
                open_hover(app, service, Surface::Shell).await?;
                let expression=format!("(()=>{{const s=document.querySelector('.native-surface.preview');if(!s||s.classList.contains('widget-palette')||document.documentElement.dataset.theme!=='{theme}')return false;const c=getComputedStyle(s),ref={expected};const rgb=h=>'rgb('+[1,3,5].map(i=>parseInt(h.slice(i,i+2),16)).join(', ')+')';return c.borderTopColor===rgb(s.style.getPropertyValue('--widget-frame'))&&c.backgroundImage.includes(rgb(ref['--wash']))&&c.backgroundImage.includes(rgb(ref['--shell']));}})()");
                frontend_assert(
                    app,
                    "preview",
                    &expression,
                    &format!("original-{theme}-{mode:?}-hover-cascade"),
                )
                .await?;
                service.dispatch(Action::Window(Input::Request(
                    Surface::Preview,
                    Request::Collapse,
                )));
            }
            let mut custom = service.snapshot().await?.settings;
            let pair = custom
                .widget_appearance
                .quota_customizations
                .entry(PresetId::Original)
                .or_default();
            let selected = if dark {
                &mut pair.dark
            } else {
                &mut pair.light
            };
            selected.enabled = true;
            selected.five_hour = Some(if dark { "#34D399" } else { "#047857" }.into());
            selected.weekly = Some(if dark { "#60A5FA" } else { "#005FCC" }.into());
            save_settings(service, custom).await?;
            let reading = if mode == DisplayMode::Floating {
                ".period-weekly .quota-reading strong"
            } else if mode == DisplayMode::ScreenEdge {
                ".weekly .ring-reading>strong"
            } else {
                ".hud-quota.weekly>strong"
            };
            let color = if dark {
                "rgb(96, 165, 250)"
            } else {
                "rgb(0, 95, 204)"
            };
            let expression=format!("(()=>{{const s=document.querySelector('.shell'),r=document.querySelector('{reading}');return s&&!s.classList.contains('widget-palette')&&r&&getComputedStyle(r).color==='{color}';}})()");
            frontend_assert(
                app,
                "shell",
                &expression,
                &format!("original-{theme}-{mode:?}-custom-quota-cascade"),
            )
            .await?;
        }
        service.dispatch(Action::Window(Input::Open {
            standard: true,
            settings: false,
        }));
        wait(app, service, "original-dashboard", |s| {
            s.presentation.detail == DetailState::Window && visible(app, "main")
        })
        .await?;
        let theme = if dark { "dark" } else { "light" };
        let expected = serde_json::to_string(&reference[theme]).unwrap();
        let expression=format!("(()=>{{const frame=document.querySelector('.app-frame'),header=document.querySelector('.app-header'),card=document.querySelector('.quota-card');if(!frame||!header||!card||document.documentElement.dataset.theme!=='{theme}')return false;const ref={expected},rgb=h=>'rgb('+[1,3,5].map(i=>parseInt(h.slice(i,i+2),16)).join(', ')+')';return getComputedStyle(frame).backgroundColor===rgb(ref['--canvas'])&&getComputedStyle(header).backgroundColor===rgb(ref['--surface'])&&getComputedStyle(card).backgroundColor===rgb(ref['--surface']);}})()");
        frontend_assert(
            app,
            "main",
            &expression,
            &format!("original-{theme}-dashboard-cascade"),
        )
        .await?;
        service.dispatch(Action::Window(Input::Close));
    }
    Ok(())
}
async fn settings_preview_check(app: &AppHandle, service: &Service) -> Result<(), String> {
    use crate::widget_appearance::{PresetId, WidgetAppearance};
    let mut settings = service.snapshot().await?.settings;
    settings.mode = DisplayMode::Floating;
    settings.theme_preference = ThemePreference::Light;
    settings.windows.snap_gap_logical_px = 8;
    settings.widget_appearance = WidgetAppearance {
        preset_id: Some(PresetId::Original),
        ..Default::default()
    };
    save_settings(service, settings).await?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Tray,
        Request::ResetPosition,
    )));
    service.dispatch(Action::Window(Input::Open {
        standard: false,
        settings: true,
    }));
    wait(app, service, "settings-preview-session", |s| {
        s.appearance.preview_session.is_some() && visible(app, "main")
    })
    .await?;
    let saved = service.snapshot().await?.settings;
    let main = app.get_webview_window("main").ok_or("preview-main")?;
    frontend_assert(app, "main", "document.querySelector('#show-widget-border')?.checked===true&&![...document.querySelectorAll('.settings-form label,.settings-form summary')].some(e=>/transparency/i.test(e.textContent))", "border-control-visible-transparency-controls-absent").await?;
    main.eval("document.querySelector('#show-widget-border').click()")
        .map_err(|_| "preview-border")?;
    wait(app, service, "preview-border-off-without-save", |s| {
        s.appearance.widget.frame_border == "transparent"
            && s.settings.widget_appearance.show_border
            && s.settings.revision == saved.revision
    })
    .await?;
    main.eval("document.querySelector('[data-preset=glacier]').click()")
        .map_err(|_| "preview-click")?;
    wait(app, service, "preview-preset", |s| {
        s.appearance.widget.palette["background"] == "#E8F8FF"
    })
    .await?;
    require(
        service.snapshot().await?.settings.revision == saved.revision,
        "preview-does-not-save",
    )?;
    main.eval("document.querySelector('input[name=theme][value=dark]').click()")
        .map_err(|_| "preview-theme")?;
    wait(app, service, "preview-dark", |s| {
        s.appearance.resolved_theme == "dark"
            && s.appearance.widget.palette["background"] == "#09283B"
            && s.appearance.widget.frame_border == "transparent"
    })
    .await?;
    frontend_assert(app,"main","document.querySelector('.preset-grid').parentElement.textContent.includes('Editing Dark colors for Glacier')&&!document.querySelector('.appearance-preview')","active-theme-editor-no-embedded-preview").await?;
    main.eval("window.__themeFont=document.documentElement.style.fontSize;document.documentElement.style.fontSize='24px'").map_err(|_|"theme-text-scale")?;
    frontend_assert(app,"main","(()=>{document.documentElement.style.fontSize='24px';const g=document.querySelector('.preset-grid');return getComputedStyle(document.documentElement).fontSize==='24px'&&g&&g.scrollWidth<=g.clientWidth+1&&[...g.querySelectorAll('button')].every(b=>b.scrollWidth<=b.clientWidth+1);})()","theme-selector-fits-at-synthetic-150-percent-text").await?;
    main.eval("document.documentElement.style.fontSize=window.__themeFont")
        .map_err(|_| "theme-text-scale-restore")?;
    main.eval("[...document.querySelectorAll('label')].find(el=>el.textContent==='Use custom quota colors').querySelector('input').click()").map_err(|_|"preview-custom")?;
    frontend_assert(
        app,
        "main",
        "document.querySelectorAll('.quota-swatches').length===2",
        "custom-controls-opt-in",
    )
    .await?;
    main.eval("document.querySelector('.quota-swatches button[aria-label*=\"#34D399\"]').click()")
        .map_err(|_| "preview-swatch")?;
    wait(app, service, "preview-color", |s| {
        s.appearance.widget.palette["fiveHour"] == "#34D399"
    })
    .await?;
    main.eval("document.querySelectorAll('.quota-swatches')[0].querySelector('[data-color=\"#FACC15\"]').click()").map_err(|_|"preview-warm-colors")?;
    wait(app, service, "preview-yellow-color", |s| {
        s.appearance.widget.palette["fiveHour"] == "#FACC15"
    })
    .await?;
    main.eval("document.querySelectorAll('.quota-swatches')[1].querySelector('[data-color=\"#FB923C\"]').click()").map_err(|_|"preview-warm-colors")?;
    wait(app, service, "preview-independent-warm-colors", |s| {
        s.appearance.widget.palette["fiveHour"] == "#FACC15"
            && s.appearance.widget.palette["weekly"] == "#FB923C"
    })
    .await?;
    main.eval("document.querySelectorAll('.quota-swatches')[0].querySelector('[aria-checked=true]').dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight',bubbles:true}))").map_err(|_|"preview-color-keyboard")?;
    wait(app, service, "preview-keyboard-next-color", |s| {
        s.appearance.widget.palette["fiveHour"] == "#FEF08A"
            && s.appearance.widget.palette["weekly"] == "#FB923C"
    })
    .await?;
    frontend_assert(app,"main","document.querySelectorAll('.quota-swatches')[0].querySelector('[aria-checked=true]')?.getAttribute('data-color')==='#FEF08A'&&document.querySelectorAll('.quota-swatches')[0].querySelector('[aria-checked=true]')?.tabIndex===0&&document.querySelectorAll('.quota-swatches button[aria-checked=true]').length===2","bright-colors-keyboard-tab-stop-and-independent-selection").await?;
    let position = main.outer_position().map_err(|_| "preview-position")?;
    main.eval("(()=>{const range=document.querySelector('.snap-gap input[type=range]');range.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,pointerId:1}));const input=document.querySelector('#snap-gap');Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,'48');input.dispatchEvent(new Event('input',{bubbles:true}));})()").map_err(|_|"preview-gap")?;
    wait(app, service, "preview-gap-moves-widget", |s| {
        s.appearance.placement_constraints.margins[0] >= 48.0
    })
    .await?;
    require(
        main.outer_position().map_err(|_| "preview-position")? == position,
        "gap-drag-freezes-settings-popup",
    )?;
    main.eval("(()=>{const input=document.querySelector('#snap-gap');Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(input,'');input.dispatchEvent(new Event('input',{bubbles:true}));})()").map_err(|_|"preview-incomplete-gap")?;
    frontend_assert(app,"main","document.querySelector('#snap-gap').value===''&&document.querySelector('#snap-gap').getAttribute('aria-invalid')==='true'","incomplete-gap-stays-invalid").await?;
    main.eval("document.querySelector('.snap-gap input[type=range]').dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:1}))").map_err(|_|"preview-gap-release")?;
    wait(app, service, "invalid-gap-still-releases-popup", |s| {
        s.display_preview
            .as_ref()
            .and_then(|p| p.patch.as_ref())
            .is_some_and(|p| !p.freeze_popup && p.snap_gap_logical_px == 48)
    })
    .await?;
    require(
        service
            .snapshot()
            .await?
            .settings
            .windows
            .snap_gap_logical_px
            == 8,
        "preview-gap-not-persisted",
    )?;
    main.eval("[...document.querySelectorAll('.save-bar button')].find(el=>el.textContent==='Cancel').click()").map_err(|_|"preview-cancel")?;
    wait(app, service, "preview-cancel-restores", |s| {
        s.appearance.resolved_theme == "light"
            && s.appearance.widget.palette["background"] == "#EAF2FF"
            && s.appearance.placement_constraints.margins[0] < 48.0
            && s.appearance.widget.frame_border != "transparent"
    })
    .await?;
    require(
        service.snapshot().await?.settings.revision == saved.revision,
        "cancel-keeps-saved-revision",
    )?;
    frontend_assert(
        app,
        "main",
        "document.querySelector('#show-widget-border')?.checked===true",
        "cancel-restores-border-control",
    )
    .await?;
    main.eval("document.querySelector('#show-widget-border').click()")
        .map_err(|_| "save-border-off")?;
    wait(app, service, "save-border-draft", |s| {
        s.appearance.widget.frame_border == "transparent"
    })
    .await?;
    main.eval("document.querySelector('[data-preset=matcha]').click()")
        .map_err(|_| "preview-matcha")?;
    wait(app, service, "preview-matcha", |s| {
        s.appearance.widget.palette["background"] == "#F0F6E4"
    })
    .await?;
    main.eval("[...document.querySelectorAll('label')].find(el=>el.textContent==='Use custom quota colors').querySelector('input').click()").map_err(|_|"save-custom-opt-in")?;
    frontend_assert(
        app,
        "main",
        "document.querySelectorAll('.quota-swatches').length===2",
        "save-custom-controls-visible",
    )
    .await?;
    main.eval("document.querySelectorAll('.quota-swatches')[0].querySelector('[data-color=\"#FACC15\"]').click()").map_err(|_|"save-warm-colors")?;
    wait(app, service, "save-yellow-draft", |s| {
        s.appearance.widget.palette["fiveHour"] == "#FACC15"
    })
    .await?;
    main.eval("document.querySelectorAll('.quota-swatches')[1].querySelector('[data-color=\"#F97316\"]').click()").map_err(|_|"save-warm-colors")?;
    wait(app, service, "save-warm-draft", |s| {
        s.appearance.widget.palette["fiveHour"] == "#FACC15"
            && s.appearance.widget.palette["weekly"] == "#F97316"
    })
    .await?;
    main.eval("document.querySelector('.save-bar button[type=submit]').click()")
        .map_err(|_| "preview-save")?;
    wait(app, service, "preview-save-once", |s| {
        s.settings.revision == saved.revision + 1
            && s.settings.widget_appearance.preset_id == Some(PresetId::Matcha)
    })
    .await?;
    let saved_colors = service
        .snapshot()
        .await?
        .settings
        .widget_appearance
        .quota_customizations[&PresetId::Matcha]
        .light
        .clone();
    require(
        saved_colors.enabled
            && saved_colors.five_hour.as_deref() == Some("#FACC15")
            && saved_colors.weekly.as_deref() == Some("#F97316"),
        "save-persists-bright-5h-and-weekly-colors",
    )?;
    let snapshot = service.snapshot().await?;
    let (_, reopened) =
        crate::storage::Store::open(std::path::Path::new(&snapshot.storage.active_directory));
    require(
        !snapshot.settings.widget_appearance.show_border
            && !reopened.settings.widget_appearance.show_border,
        "save-persists-border-off-on-storage-reopen",
    )?;
    service.dispatch(Action::Window(Input::Close));
    wait(app, service, "preview-close-retires-session", |s| {
        s.appearance.preview_session.is_none() && !visible(app, "main")
    })
    .await?;
    require(
        service
            .snapshot()
            .await?
            .settings
            .widget_appearance
            .preset_id
            == Some(PresetId::Matcha),
        "close-retains-saved-preset",
    )?;
    Ok(())
}

struct PaintBackdrop {
    app: AppHandle,
    handle: usize,
}
impl Drop for PaintBackdrop {
    fn drop(&mut self) {
        let handle = self.handle;
        let _ = self.app.run_on_main_thread(move || unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(handle as _);
        });
    }
}
unsafe extern "system" fn paint_backdrop_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use windows_sys::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};
    unsafe {
        match message {
            WM_NCCREATE => {
                let create = &*(lparam as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
                1
            }
            WM_ERASEBKGND => 1,
            WM_PAINT => {
                let mut paint = std::mem::zeroed();
                let dc = BeginPaint(hwnd, &mut paint);
                let mut rect = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);
                let brush = GetStockObject(if GetWindowLongPtrW(hwnd, GWLP_USERDATA) == 1 {
                    WHITE_BRUSH
                } else {
                    BLACK_BRUSH
                });
                FillRect(dc, &rect, brush);
                EndPaint(hwnd, &paint);
                0
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}
unsafe fn capture_composed_pixel(
    point: windows_sys::Win32::Foundation::POINT,
) -> Result<u32, &'static str> {
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
    unsafe {
        // GetDC coordinates are virtualized when a WebView2 callback has changed
        // the thread context. Our verified window rectangles are physical pixels.
        let module = GetModuleHandleW("user32.dll\0".encode_utf16().collect::<Vec<_>>().as_ptr());
        let setter = GetProcAddress(module, c"SetThreadDpiAwarenessContext".as_ptr().cast()).map(
            |function| {
                std::mem::transmute::<
                    unsafe extern "system" fn() -> isize,
                    unsafe extern "system" fn(isize) -> isize,
                >(function)
            },
        );
        let previous_dpi = setter.map_or(0, |set| set(-4));
        let source = GetDC(std::ptr::null_mut());
        let mut clip = std::mem::zeroed();
        let region = GetClipBox(source, &mut clip);
        let memory = CreateCompatibleDC(source);
        let bitmap = CreateCompatibleBitmap(source, 1, 1);
        let previous = SelectObject(memory, bitmap);
        let copied = BitBlt(
            memory,
            0,
            0,
            1,
            1,
            source,
            point.x,
            point.y,
            SRCCOPY | CAPTUREBLT,
        );
        let value = if copied != 0 {
            GetPixel(memory, 0, 0)
        } else {
            u32::MAX
        };
        record(format!("native-check composed-capture dc={} region={region} clip={}x{} copied={copied} value={value:08X}",!source.is_null(),clip.right-clip.left,clip.bottom-clip.top));
        SelectObject(memory, previous);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(std::ptr::null_mut(), source);
        if previous_dpi != 0 {
            if let Some(set) = setter {
                set(previous_dpi);
            }
        }
        if region == NULLREGION || value == u32::MAX {
            Err("native-capture-unavailable")
        } else {
            Ok(value)
        }
    }
}
async fn composition_probe(
    app: &AppHandle,
    shell: &tauri::WebviewWindow,
    white: bool,
    sample: Option<[f64; 2]>,
) -> Result<[u8; 3], String> {
    use windows_sys::Win32::{Graphics::Gdi::*, UI::WindowsAndMessaging::*};
    let position = shell.outer_position().map_err(|_| "paint-position")?;
    let size = shell.outer_size().map_err(|_| "paint-size")?;
    let scale = shell.scale_factor().map_err(|_| "paint-scale")?;
    let client_position = shell
        .inner_position()
        .map_err(|_| "paint-client-position")?;
    let shell_handle = shell.hwnd().map_err(|_| "paint-handle")?.0 as usize;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let class = "CodexUsageValidationBackdrop\0"
            .encode_utf16()
            .collect::<Vec<_>>();
        static REGISTERED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        let registered = *REGISTERED.get_or_init(|| unsafe {
            let definition = WNDCLASSW {
                lpfnWndProc: Some(paint_backdrop_proc),
                hInstance: windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
                    std::ptr::null(),
                ),
                lpszClassName: class.as_ptr(),
                ..std::mem::zeroed()
            };
            RegisterClassW(&definition) != 0
        });
        if !registered {
            let _ = send.send(0);
            return;
        }
        let handle = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                position.x - 8,
                position.y - 8,
                size.width as i32 + 16,
                size.height as i32 + 16,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()),
                usize::from(white) as *const std::ffi::c_void,
            )
        };
        if !handle.is_null() {
            unsafe {
                SetWindowPos(
                    handle,
                    shell_handle as _,
                    position.x - 8,
                    position.y - 8,
                    size.width as i32 + 16,
                    size.height as i32 + 16,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                UpdateWindow(handle);
            }
        }
        let _ = send.send(handle as usize);
    })
    .map_err(|_| "paint-backdrop-thread")?;
    let handle = receive.await.map_err(|_| "paint-backdrop")?;
    if handle == 0 {
        return Err("paint-backdrop".into());
    }
    let _backdrop = PaintBackdrop {
        app: app.clone(),
        handle,
    };
    tokio::time::sleep(Duration::from_millis(180)).await;
    let (send, receive) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let color = unsafe {
            let point = windows_sys::Win32::Foundation::POINT {
                x: sample.map_or(position.x+(3.0*scale).round() as i32,|p|client_position.x+(p[0]*scale).round() as i32),
                y: sample.map_or(position.y+(size.height/2) as i32,|p|client_position.y+(p[1]*scale).round() as i32),
            };
            let hit = WindowFromPoint(point);
            if GetAncestor(hit, GA_ROOT) != shell_handle as _ {
                Err("paint-sample-occluded")
            } else {
                let outside = windows_sys::Win32::Foundation::POINT {
                    x: position.x - 4,
                    y: position.y + (size.height / 2) as i32,
                };
                let backdrop_hit = GetAncestor(WindowFromPoint(outside), GA_ROOT);
                let backdrop = if backdrop_hit == handle as _ {
                    let dc = GetDC(handle as _);
                    let mut local=outside;
                    ScreenToClient(handle as _,&mut local);
                    let value=GetPixel(dc,local.x,local.y);
                    ReleaseDC(handle as _,dc);
                    value
                } else {
                    u32::MAX
                };
                let color = capture_composed_pixel(point);
                let expected = if white { 0x00FFFFFF } else { 0 };
                record(format!("native-check own-backdrop-valid={} root={} expected={expected:08X} actual={backdrop:08X} sample={color:?}", backdrop == expected, backdrop_hit == handle as _));
                if backdrop == expected { color } else { Err("paint-backdrop-unverified") }
            }
        };
        let _ = send.send(color);
    })
    .map_err(|_| "paint-sample-thread")?;
    let color = receive.await.map_err(|_| "paint-sample")??;
    // Only this app's decorative shell over an app-owned solid backdrop is sampled.
    // The difference cancels gradient tint and measures CSS background alpha.
    Ok([
        (color & 255) as u8,
        ((color >> 8) & 255) as u8,
        ((color >> 16) & 255) as u8,
    ])
}

// Inject coordinator drag completion without OS mouse input. Actual cross-DPI
// pointer drags remain a manual runtime gate.
async fn edge_docking(app: &AppHandle, service: &Service) -> Result<(), String> {
    use crate::{geometry::Rect, model::EdgeSide};
    service.dispatch(Action::Window(Input::Close));
    let mut settings = service.snapshot().await?.settings;
    settings.mode = DisplayMode::ScreenEdge;
    settings.windows.lock_position = false;
    settings.windows.suppress_fullscreen = false;
    save_settings(service, settings).await?;
    wait(app, service, "edge-drag-hydrated", |_| {
        visible(app, "shell")
    })
    .await?;
    let shell = app.get_webview_window("shell").ok_or("drag-shell")?;
    let handle = shell.hwnd().map_err(|_| "drag-handle")?.0 as usize;
    let monitors = app.available_monitors().map_err(|_| "drag-monitors")?;
    for monitor in &monitors {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        let work = Rect {
            x: area.position.x as f64,
            y: area.position.y as f64,
            width: area.size.width as f64,
            height: area.size.height as f64,
        };
        for right in [false, true] {
            let revision = service.snapshot().await?.settings.revision;
            service.dispatch(Action::Window(Input::InspectDragStart));
            wait(app, service, "edge-drag-start", |s| s.presentation.dragging).await?;
            let size = shell.outer_size().map_err(|_| "drag-size")?;
            let x = if right {
                work.right() - size.width as f64 - 60.0 * scale
            } else {
                work.x + 60.0 * scale
            };
            let bounds = Rect {
                x,
                y: work.y + 100.0 * scale,
                width: size.width as f64,
                height: size.height as f64,
            };
            service.dispatch(Action::Window(Input::DragFinished {
                window: handle,
                bounds: Some(bounds),
                pointer: Some((
                    if right {
                        work.right() - 30.0 * scale
                    } else {
                        work.x + 30.0 * scale
                    },
                    bounds.y,
                )),
            }));
            let state = wait(app, service, "edge-drag-docked", |s| {
                !s.presentation.dragging && s.settings.revision == revision + 1
            })
            .await?;
            require(
                state.settings.windows.edge_side
                    == if right {
                        EdgeSide::Right
                    } else {
                        EdgeSide::Left
                    },
                "edge-drag-saves-side",
            )?;
            let placement = state
                .settings
                .windows
                .placements
                .get("screen-edge")
                .ok_or("drag-placement")?;
            require(
                placement.monitor.as_ref() == monitor.name(),
                "edge-drag-saves-drop-monitor",
            )?;
            require((placement.y - 100.0).abs() <= 1.0, "edge-drag-saves-height")?;
            require(
                state.presentation.detail == DetailState::Closed,
                "edge-drag-no-overview",
            )?;
            let position = shell.outer_position().map_err(|_| "drag-position")?;
            let size = shell.outer_size().map_err(|_| "drag-size")?;
            let gap = if right {
                work.right() - position.x as f64 - size.width as f64
            } else {
                position.x as f64 - work.x
            };
            require(
                (gap - 8.0 * scale).abs() <= 2.0,
                "edge-drag-docks-with-inset",
            )?;
        }
    }
    // A lock prevents beginning a drag and an unmatched completion cannot save.
    let mut settings = service.snapshot().await?.settings;
    settings.windows.lock_position = true;
    save_settings(service, settings).await?;
    let revision = service.snapshot().await?.settings.revision;
    service.dispatch(Action::Window(Input::InspectDragStart));
    service.dispatch(Action::Window(Input::DragFinished {
        window: handle,
        bounds: Some(Rect {
            x: 300.0,
            y: 200.0,
            width: 60.0,
            height: 200.0,
        }),
        pointer: Some((300.0, 200.0)),
    }));
    let state = service.snapshot().await?;
    require(
        !state.presentation.dragging && state.settings.revision == revision,
        "edge-drag-lock-rejects-completion",
    )?;
    let mut settings = state.settings;
    settings.windows.lock_position = false;
    save_settings(service, settings).await?;
    Ok(())
}

async fn open_hover(app: &AppHandle, service: &Service, source: Surface) -> Result<(), String> {
    let state = service.snapshot().await?;
    service.dispatch(Action::Window(Input::Pointer {
        generation: state.presentation.generation,
        source,
        trigger: false,
        preview: false,
        corridor: false,
    }));
    for _ in 0..160 {
        let state = service.snapshot().await?;
        service.dispatch(Action::Window(Input::Pointer {
            generation: state.presentation.generation,
            source,
            trigger: true,
            preview: false,
            corridor: false,
        }));
        if state.presentation.preview_source == source
            && state.presentation.shell == ShellState::Preview
            && visible(app, "preview")
        {
            return Ok(());
        }
        if let Some(error) = state.window_diagnostic {
            return Err(format!("hover-hydration: {error}"));
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(format!("hover-hydration-timeout source={source:?}"))
}

async fn preview_sizing(app: &AppHandle, service: &Service, source: Surface) -> Result<(), String> {
    let preview = app.get_webview_window("preview").ok_or("preview-missing")?;
    let mut stable = 0;
    let mut previous = None;
    for _ in 0..100 {
        let state = service.snapshot().await?;
        service.dispatch(Action::Window(Input::Pointer {
            generation: state.presentation.generation,
            source,
            trigger: true,
            preview: false,
            corridor: false,
        }));
        let (send, receive) = tokio::sync::oneshot::channel();
        service
            .0
            .send(Action::PreviewContentSize(send))
            .await
            .map_err(|_| "service-stopped")?;
        let content = receive.await.map_err(|_| "service-stopped")?;
        let client = preview.inner_size().map_err(|_| "preview-client")?;
        let scale = preview.scale_factor().map_err(|_| "preview-scale")?;
        let measured = content.map(|c| ((c.0 * scale).ceil() as u32, (c.1 * scale).ceil() as u32));
        if measured == Some((client.width, client.height))
            && measured == previous
            && visible(app, "preview")
        {
            stable += 1;
            if stable == 5 {
                break;
            }
        } else {
            stable = 0;
        }
        previous = measured;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    require(stable == 5, "preview-client-sizing-stable")?;
    require(
        crate::native_windows::has_rounded_corners(&preview),
        "preview-rounded-region",
    )?;
    if source == Surface::Tray {
        let size = preview.inner_size().map_err(|_| "preview-client")?;
        let scale = preview.scale_factor().map_err(|_| "preview-scale")?;
        require(
            (size.width as f64 / scale - 260.0).abs() < 1.0 && size.height as f64 / scale >= 76.0,
            "tray-preview-baseline",
        )?;
    } else {
        let state = service.snapshot().await?;
        let shell = app
            .get_webview_window("shell")
            .ok_or("hover-shell-missing")?;
        let shell_position = shell.outer_position().map_err(|_| "hover-shell-position")?;
        let shell_size = shell.outer_size().map_err(|_| "hover-shell-size")?;
        let position = preview.outer_position().map_err(|_| "hover-position")?;
        let size = preview.outer_size().map_err(|_| "hover-size")?;
        let monitor = app
            .monitor_from_point(shell_position.x as f64, shell_position.y as f64)
            .map_err(|_| "hover-monitor")?
            .ok_or("hover-monitor")?;
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let work = crate::geometry::Rect {
            x: area.position.x as f64,
            y: area.position.y as f64,
            width: area.size.width as f64,
            height: area.size.height as f64,
        };
        require(
            position.x as f64 >= work.x
                && position.y as f64 >= work.y
                && (position.x as f64 + size.width as f64) <= work.right() + 1.0
                && (position.y as f64 + size.height as f64) <= work.bottom() + 1.0,
            "hover-stays-in-work-area",
        )?;
        if state.settings.mode == DisplayMode::CompactHud {
            let centered = (shell_position.x as f64
                + (shell_size.width as f64 - size.width as f64) / 2.0)
                .clamp(work.x, work.right() - size.width as f64);
            require(
                (position.x as f64 - centered).abs() <= 2.0,
                "hud-hover-centered-or-boundary-clamped",
            )?;
        } else if state.settings.mode == DisplayMode::ScreenEdge {
            let gap = if state.settings.windows.edge_side == crate::model::EdgeSide::Right {
                shell_position.x as f64 - (position.x as f64 + size.width as f64)
            } else {
                position.x as f64 - (shell_position.x as f64 + shell_size.width as f64)
            };
            require(
                (gap - 8.0 * scale).abs() <= 2.0,
                "edge-hover-eight-logical-pixel-gap",
            )?;
        }
    }
    Ok(())
}

async fn reveal_check(app: &AppHandle, service: &Service) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    mode(service, DisplayMode::Floating).await?;
    // A plain app-owned native window models a newly opened ordinary window.
    // It has no webview, credentials, external app, or injected OS input.
    let (send, receive) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let title: Vec<u16> = "Widget stacking validation\0".encode_utf16().collect();
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                480,
                360,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()),
                std::ptr::null(),
            )
        };
        let _ = send.send(window as usize);
    })
    .map_err(|_| "stacking-probe-thread")?;
    let blocker_handle = receive.await.map_err(|_| "stacking-probe-create")?;
    if blocker_handle == 0 {
        return Err("stacking-probe-create".into());
    }
    service.dispatch(Action::Window(Input::Close));
    for topmost in [false, true] {
        for selected in [
            DisplayMode::Floating,
            DisplayMode::ScreenEdge,
            DisplayMode::CompactHud,
        ] {
            let mut settings = service.snapshot().await?.settings;
            settings.windows.always_on_top = topmost;
            save_settings(service, settings).await?;
            service.dispatch(Action::Window(Input::Request(
                Surface::Tray,
                Request::ToggleHidden,
            )));
            wait(app, service, "reveal-hidden", |s| {
                s.presentation.hidden && !visible(app, "shell")
            })
            .await?;
            service.dispatch(Action::Window(Input::ExternalForeground));
            let hidden = service.snapshot().await?;
            require(
                hidden.presentation.hidden && !visible(app, "shell"),
                "external-activation-preserves-hidden-state",
            )?;
            let foreground = unsafe { GetForegroundWindow() } as usize;
            service.dispatch(Action::SetMode(selected));
            wait(app, service, "reveal-selected-mode", |s| {
                !s.presentation.hidden && s.settings.mode == selected && visible(app, "shell")
            })
            .await?;
            require(
                styles(app, "shell").is_some_and(|s| (s & WS_EX_TOPMOST != 0) == topmost),
                "reveal-preserves-topmost-setting",
            )?;
            require(
                unsafe { GetForegroundWindow() } as usize == foreground,
                "reveal-preserves-foreground",
            )?;
            // Selecting an already active mode must also restore a hidden shell.
            service.dispatch(Action::Window(Input::Request(
                Surface::Tray,
                Request::ToggleHidden,
            )));
            wait(app, service, "reselect-hidden", |s| s.presentation.hidden).await?;
            service.dispatch(Action::SetMode(selected));
            wait(app, service, "reselect-visible", |s| {
                !s.presentation.hidden && visible(app, "shell")
            })
            .await?;
            let shell = app
                .get_webview_window("shell")
                .ok_or("reveal-shell-missing")?;
            let shell_handle = shell.hwnd().map_err(|_| "reveal-shell-handle")?.0 as usize;
            let position = shell
                .outer_position()
                .map_err(|_| "reveal-shell-position")?;
            unsafe {
                SetWindowPos(
                    blocker_handle as _,
                    HWND_TOP,
                    position.x,
                    position.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
            }
            require(
                handles_above(shell_handle, blocker_handle) == topmost,
                "new-window-respects-widget-topmost",
            )?;
            service.dispatch(Action::SetMode(selected));
            // Wait for the new generation's hydration and one-shot raise.
            let mut raised = false;
            for _ in 0..100 {
                if handles_above(shell_handle, blocker_handle) {
                    raised = true;
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            require(raised, "explicit-selection-raises-above-new-window")?;
            unsafe {
                SetWindowPos(
                    blocker_handle as _,
                    HWND_TOP,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
            service.dispatch(Action::Window(Input::Environment));
            let _ = service.snapshot().await?;
            require(
                handles_above(shell_handle, blocker_handle) == topmost,
                "reconciliation-preserves-window-order",
            )?;
            if !topmost {
                service.dispatch(Action::Window(Input::ExternalForeground));
                let _ = service.snapshot().await?;
                require(
                    !handles_above(shell_handle, blocker_handle),
                    "external-activation-respects-disabled-topmost",
                )?;
            }
            if topmost {
                let foreground = unsafe { GetForegroundWindow() } as usize;
                unsafe {
                    SetWindowPos(
                        blocker_handle as _,
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
                require(
                    !handles_above(shell_handle, blocker_handle)
                        && styles(app, "shell").is_some_and(|s| s & WS_EX_TOPMOST != 0),
                    "competing-topmost-window-covers-still-topmost-widget",
                )?;
                service.dispatch(Action::Window(Input::ExternalForeground));
                let _ = service.snapshot().await?;
                require(
                    handles_above(shell_handle, blocker_handle),
                    "external-activation-restores-topmost-order",
                )?;
                require(
                    unsafe { GetForegroundWindow() } as usize == foreground,
                    "external-activation-repair-preserves-foreground",
                )?;
                unsafe {
                    SetWindowPos(
                        blocker_handle as _,
                        HWND_NOTOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                }
                let foreground = unsafe { GetForegroundWindow() } as usize;
                let copy = shell.clone();
                let (send, receive) = tokio::sync::oneshot::channel();
                app.run_on_main_thread(move || {
                    let result = copy.set_shadow(true).and_then(|_| copy.set_shadow(false));
                    let kept = unsafe {
                        GetWindowLongPtrW(shell_handle as _, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST
                            != 0
                    };
                    // Simulate a later native demotion: own-window messages must
                    // schedule a repair without a timer or foreground request.
                    unsafe {
                        SetWindowPos(
                            shell_handle as _,
                            HWND_NOTOPMOST,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    }
                    let _ = send.send(result.is_ok() && kept);
                })
                .map_err(|_| "style-regression-thread")?;
                require(
                    receive.await.map_err(|_| "style-regression-completion")?,
                    "framework-style-rewrite-preserves-topmost",
                )?;
                wait(app, service, "event-driven-topmost-repair", |_| {
                    styles(app, "shell").is_some_and(|s| s & WS_EX_TOPMOST != 0)
                })
                .await?;
                require(
                    unsafe { GetForegroundWindow() } as usize == foreground,
                    "topmost-repair-preserves-foreground",
                )?;
            }
            unsafe {
                ShowWindow(blocker_handle as _, SW_HIDE);
            }
        }
    }
    app.run_on_main_thread(move || unsafe {
        DestroyWindow(blocker_handle as _);
    })
    .map_err(|_| "stacking-probe-destroy")?;
    require(true, "hidden-mode-selection-and-reselection")
}

fn handles_above(window: usize, anchor: usize) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindow, GW_HWNDPREV};
    let mut previous = unsafe { GetWindow(anchor as _, GW_HWNDPREV) };
    while !previous.is_null() {
        if previous as usize == window {
            return true;
        }
        previous = unsafe { GetWindow(previous, GW_HWNDPREV) };
    }
    false
}

async fn popup_stacking(app: &AppHandle, service: &Service) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, WS_EX_TOPMOST};
    for (mode, topmost) in [
        (DisplayMode::Floating, false),
        (DisplayMode::Floating, true),
        (DisplayMode::CompactHud, false),
        (DisplayMode::CompactHud, true),
    ] {
        service.dispatch(Action::Window(Input::Close));
        let mut settings = service.snapshot().await?.settings;
        settings.mode = mode;
        settings.windows.always_on_top = topmost;
        save_settings(service, settings).await?;
        wait(app, service, "popup-shell-hydration", |_| {
            visible(app, "shell")
        })
        .await?;
        let foreground = unsafe { GetForegroundWindow() } as usize;
        service.dispatch(Action::Window(Input::InspectPopup));
        wait(app, service, "popup-stacking-hydration", |s| {
            s.presentation.detail == DetailState::Popup && visible(app, "main")
        })
        .await?;
        let popup = app.get_webview_window("main").ok_or("popup-missing")?;
        let shell = app
            .get_webview_window("shell")
            .ok_or("popup-shell-missing")?;
        require(
            crate::native_windows::is_above(&popup, &shell),
            "popup-above-shell",
        )?;
        require(
            styles(app, "main").is_some_and(|s| (s & WS_EX_TOPMOST != 0) == topmost),
            "popup-matches-anchor-topmost",
        )?;
        require(
            unsafe { GetForegroundWindow() } as usize == foreground,
            "popup-geometry-check-preserves-foreground",
        )?;
        if mode == DisplayMode::CompactHud {
            let p = shell.outer_position().map_err(|_| "hud-position")?;
            let size = shell.outer_size().map_err(|_| "hud-size")?;
            let mp = popup.outer_position().map_err(|_| "popup-position")?;
            let ms = popup.outer_size().map_err(|_| "popup-size")?;
            let monitor = shell
                .current_monitor()
                .map_err(|_| "hud-monitor")?
                .ok_or("hud-monitor")?;
            let a = monitor.work_area();
            let expected = crate::geometry::centered_panel(
                crate::geometry::Rect {
                    x: f64::from(p.x),
                    y: f64::from(p.y),
                    width: f64::from(size.width),
                    height: f64::from(size.height),
                },
                f64::from(ms.width),
                f64::from(ms.height),
                8.0 * monitor.scale_factor(),
                crate::geometry::Rect {
                    x: f64::from(a.position.x),
                    y: f64::from(a.position.y),
                    width: f64::from(a.size.width),
                    height: f64::from(a.size.height),
                },
            );
            require(
                (f64::from(mp.x) - expected.x).abs() <= 1.0
                    && (f64::from(mp.y) - expected.y).abs() <= 1.0,
                "hud-click-popup-centered-and-clamped",
            )?;
        }
        service.dispatch(Action::Window(Input::Close));
    }
    Ok(())
}

async fn floating_close_to_tray(app: &AppHandle, service: &Service) -> Result<(), String> {
    if !service.snapshot().await?.tray_available {
        return Err("floating-close-needs-tray".into());
    }
    service.dispatch(Action::Window(Input::Close));
    mode(service, DisplayMode::Floating).await?;
    wait(app, service, "floating-before-close", |s| {
        s.settings.mode == DisplayMode::Floating && visible(app, "shell")
    })
    .await?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Shell,
        Request::CloseToTray {},
    )));
    wait(app, service, "floating-close-to-tray", |s| {
        s.settings.mode == DisplayMode::SystemTray
            && !visible(app, "shell")
            && !visible(app, "preview")
    })
    .await?;
    require(true, "floating-close-switches-to-tray")
}

async fn notification_check(app: &AppHandle, service: &Service) -> Result<(), String> {
    let before = service.snapshot().await?;
    require(
        !before.settings.reminders.enabled
            && before.settings.reminders.silent
            && !before.settings.reminders.detailed,
        "notification-test-safe-preferences",
    )?;
    let (reply, receive) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::Reminder {
            request: crate::reminders::Request::Test,
            reply,
        })
        .await
        .map_err(|_| "service-stopped")?;
    let state = receive.await.map_err(|_| "service-stopped")??;
    let delivery = state
        .reminders
        .history
        .last()
        .ok_or("notification-test-history-missing")?;
    require(
        delivery.status == crate::reminders::Status::OutcomeUnknown
            && delivery.events.len() == 1
            && delivery.events[0].category == crate::reminders::Category::Test
            && delivery.events[0].remaining.is_none()
            && delivery.events[0].binding.is_none(),
        "notification-test-recorded-unconfirmed",
    )?;
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "check-storage-path")?
        .join("native-window-check");
    let (_, stored) = crate::storage::Store::open(&directory);
    require(
        stored.diagnostic.is_none()
            && stored.reminders.history.last().is_some_and(|d| {
                d.id == delivery.id
                    && d.status == crate::reminders::Status::OutcomeUnknown
                    && d.retry_at.is_none()
            }),
        "notification-test-outcome-persisted",
    )?;
    // Allow the plugin's queued background task to run; no OS display result is exposed.
    tokio::time::sleep(Duration::from_secs(2)).await;
    Ok(())
}

async fn stress(app: &AppHandle, service: &Service) -> Result<(), String> {
    use std::collections::HashSet;
    service.dispatch(Action::Window(Input::Close));
    service.dispatch(Action::Window(Input::Request(
        Surface::Tray,
        Request::Collapse,
    )));
    wait(app, service, "stress-hidden-main", |s| {
        s.presentation.detail == DetailState::Closed && !visible(app, "main")
    })
    .await?;
    let modes = [
        DisplayMode::Floating,
        DisplayMode::ScreenEdge,
        DisplayMode::CompactHud,
        DisplayMode::SystemTray,
    ];
    let mut shell_handles = HashSet::new();
    println!("native-stress ready=pass");
    for index in 0..100 {
        let next = modes[index % 4];
        mode(service, next).await?;
        wait(app, service, "stress-mode-hydration", |s| {
            s.settings.mode == next
                && if next == DisplayMode::SystemTray {
                    !visible(app, "shell") && !visible(app, "preview")
                } else {
                    visible(app, "shell")
                }
        })
        .await?;
        if app.webview_windows().len() > 3 {
            return Err("stress-window-count".into());
        }
        if let Some(shell) = app.get_webview_window("shell") {
            shell_handles.insert(shell.hwnd().map_err(|_| "stress-shell-handle")?.0 as usize);
        }
        if (index + 1) % 25 == 0 {
            println!(
                "native-stress cycles={} views={}",
                index + 1,
                app.webview_windows().len()
            );
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    require(shell_handles.len() == 1, "stress-one-reused-shell")?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Tray,
        Request::Collapse,
    )));
    service.dispatch(Action::Window(Input::RetireHidden));
    wait(app, service, "stress-final-retirement", |_| {
        app.webview_windows().is_empty()
    })
    .await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("native-stress result=pass cycles=100");
    Ok(())
}

async fn dashboard_settled(app: &AppHandle, service: &Service) -> Result<AppState, String> {
    for _ in 0..160 {
        let (send, receive) = tokio::sync::oneshot::channel();
        service
            .0
            .send(Action::DashboardSettled(send))
            .await
            .map_err(|_| "service-stopped")?;
        if receive.await.map_err(|_| "service-stopped")? && visible(app, "main") {
            return service.snapshot().await;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err("dashboard-did-not-settle".into())
}

async fn activation_fallback(app: &AppHandle, service: &Service) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    };
    service.dispatch(Action::Window(Input::InspectPopup));
    dashboard_settled(app, service).await?;
    let main_window = app.get_webview_window("main").ok_or("activation-main")?;
    let main = main_window.hwnd().map_err(|_| "activation-hwnd")?.0 as usize;
    // A real denial cannot occur while this HWND is already foreground:
    // show_activated returns Activated in that case. Earlier keyboard tests may
    // have focused it. Release our own focus, then restore the inactive popup.
    crate::native_windows::hide(&main_window);
    crate::native_windows::show_without_activation(&main_window)?;
    let foreground = unsafe { GetForegroundWindow() } as usize;
    require(
        foreground != 0 && foreground != main,
        "activation-denial-precondition-inactive-popup",
    )?;
    // Inject the native outcome, never bypass Windows foreground restrictions.
    service.dispatch(Action::Window(Input::InspectActivationDenied));
    let recovered = dashboard_settled(app, service).await?;
    require(
        recovered.presentation.detail == DetailState::Window
            && recovered.window_diagnostic.is_none()
            && !recovered.tooltip_fallback
            && styles(app, "main").is_some_and(|flags| {
                flags & WS_EX_APPWINDOW != 0 && flags & (WS_EX_TOOLWINDOW | WS_EX_TOPMOST) == 0
            }),
        "injected-activation-denial-recovers-to-standard-window",
    )?;
    let after = unsafe { GetForegroundWindow() } as usize;
    record(format!("native-check fallback-foreground before-main={} after-main={} before-valid={} after-valid={}",foreground==main,after==main,foreground!=0,after!=0));
    require(
        after == foreground,
        "activation-fallback-preserves-foreground",
    )?;
    service.dispatch(Action::Window(Input::Close));
    Ok(())
}

async fn settle_explicit_activation(app: &AppHandle, service: &Service) -> Result<(), String> {
    let opened = dashboard_settled(app, service).await?;
    if opened.presentation.detail == DetailState::Window {
        use windows_sys::Win32::UI::WindowsAndMessaging::{WS_EX_APPWINDOW, WS_EX_TOOLWINDOW};
        require(
            opened.window_diagnostic.is_none()
                && !opened.tooltip_fallback
                && styles(app, "main")
                    .is_some_and(|s| s & WS_EX_APPWINDOW != 0 && s & WS_EX_TOOLWINDOW == 0),
            "foreground-denial-has-taskbar-recovery",
        )?;
        println!("native-check activation=standard-window-fallback; subsequent-popup-check=nonactivating");
        service.dispatch(Action::Window(Input::InspectPopup));
        dashboard_settled(app, service).await?;
    }
    Ok(())
}

async fn run(app: &AppHandle, service: &Service) -> Result<(), String> {
    require(
        crate::native_windows::identity_matches(&app.config().identifier),
        "process-app-identity",
    )?;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    wait(app, service, "initial-hydration", |_| visible(app, "main")).await?;
    require(
        styles(app, "main").is_some_and(|s| s & WS_EX_APPWINDOW != 0 && s & WS_EX_TOOLWINDOW == 0),
        "standard-style",
    )?;
    compact_sizing(app, service).await?;
    service.dispatch(Action::Window(Input::Close));
    service.dispatch(Action::Window(Input::Request(
        Surface::Tray,
        Request::Collapse,
    )));
    mode(service, DisplayMode::Floating).await?;
    wait(app, service, "floating-hydration", |s| {
        s.settings.mode == DisplayMode::Floating && visible(app, "shell") && !visible(app, "main")
    })
    .await?;
    require(
        styles(app, "shell").is_some_and(|s| s & WS_EX_TOOLWINDOW != 0 && s & WS_EX_APPWINDOW == 0),
        "floating-tool-style",
    )?;
    mode(service, DisplayMode::CompactHud).await?;
    wait(app, service, "hud-preview-trigger", |s| {
        s.settings.mode == DisplayMode::CompactHud && visible(app, "shell")
    })
    .await?;
    let foreground = unsafe { GetForegroundWindow() } as usize;
    let entry = service.snapshot().await?;
    service.dispatch(Action::Window(Input::Pointer {
        generation: entry.presentation.generation,
        source: Surface::Shell,
        trigger: false,
        preview: false,
        corridor: false,
    }));
    let mut preview_ready = false;
    for _ in 0..160 {
        let state = service.snapshot().await?;
        service.dispatch(Action::Window(Input::Pointer {
            generation: state.presentation.generation,
            source: Surface::Shell,
            trigger: true,
            preview: false,
            corridor: false,
        }));
        if state.presentation.shell == ShellState::Preview && visible(app, "preview") {
            preview_ready = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    if !preview_ready {
        let s = service.snapshot().await?;
        eprintln!("native-check preview-timeout generation={} source={:?} shell={:?} detail={:?} hidden={} suppressed={} tray_suppressed={} menu={} dragging={} windows={} diagnostic={}",s.presentation.generation,s.presentation.preview_source,s.presentation.shell,s.presentation.detail,s.presentation.hidden,s.presentation.suppressed,s.presentation.tray_suppressed,s.presentation.menu_open,s.presentation.dragging,app.webview_windows().len(),s.window_diagnostic.as_deref().unwrap_or("none"));
    }
    require(preview_ready, "preview-hydration")?;
    require(
        unsafe { GetForegroundWindow() } as usize == foreground,
        "preview-preserves-foreground",
    )?;
    require(
        styles(app, "preview")
            .is_some_and(|s| s & WS_EX_TOOLWINDOW != 0 && s & WS_EX_NOACTIVATE != 0),
        "preview-native-style",
    )?;
    let state = service.snapshot().await?;
    service.dispatch(Action::Window(Input::Pointer {
        generation: state.presentation.generation,
        source: Surface::Shell,
        trigger: false,
        preview: false,
        corridor: false,
    }));
    wait(app, service, "preview-dismissal", |s| {
        s.presentation.shell == ShellState::Compact && !visible(app, "preview")
    })
    .await?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Shell,
        Request::Activate,
    )));
    require(
        service.snapshot().await?.presentation.detail == DetailState::Closed,
        "single-click-defers-popup",
    )?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Shell,
        Request::OpenWindow,
    )));
    wait(app, service, "direct-double-click", |s| {
        s.presentation.detail == DetailState::Window && visible(app, "main")
    })
    .await?;
    tokio::time::sleep(Duration::from_millis(
        crate::native_windows::double_click_interval() + 50,
    ))
    .await;
    require(
        service.snapshot().await?.presentation.detail == DetailState::Window,
        "double-click-cancels-delayed-popup",
    )?;
    service.dispatch(Action::Window(Input::Close));
    service.dispatch(Action::Window(Input::Request(
        Surface::Shell,
        Request::Activate,
    )));
    settle_explicit_activation(app, service).await?;
    wait(app, service, "popup", |s| {
        s.presentation.detail == DetailState::Popup && visible(app, "main")
    })
    .await?;
    require(
        styles(app, "main").is_some_and(|s| s & WS_EX_TOOLWINDOW != 0 && s & WS_EX_APPWINDOW == 0),
        "popup-tool-style",
    )?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Shell,
        Request::Settings {},
    )));
    settle_explicit_activation(app, service).await?;
    wait(app, service, "widget-settings-stays-popup", |s| {
        s.presentation.detail == DetailState::Popup
            && s.dashboard_request
                .is_some_and(|r| matches!(r.page, crate::model::DashboardPage::Settings))
    })
    .await?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Main,
        Request::Promote,
    )));
    wait(app, service, "promotion", |s| {
        s.presentation.detail == DetailState::Window
            && styles(app, "main")
                .is_some_and(|s| s & WS_EX_APPWINDOW != 0 && s & WS_EX_TOOLWINDOW == 0)
    })
    .await?;
    require(true, "promotion")?;
    service.dispatch(Action::Window(Input::Close));
    mode(service, DisplayMode::ScreenEdge).await?;
    wait(app, service, "edge-hydration", |s| {
        s.settings.mode == DisplayMode::ScreenEdge && visible(app, "shell")
    })
    .await?;
    let edge_entry = service.snapshot().await?;
    service.dispatch(Action::Window(Input::Pointer {
        generation: edge_entry.presentation.generation,
        source: Surface::Shell,
        trigger: true,
        preview: false,
        corridor: false,
    }));
    wait(app, service, "edge-hover-preview", |s| {
        s.presentation.shell == ShellState::Preview && visible(app, "preview")
    })
    .await?;
    let edge_preview = service.snapshot().await?;
    service.dispatch(Action::Window(Input::Pointer {
        generation: edge_preview.presentation.generation,
        source: Surface::Shell,
        trigger: false,
        preview: false,
        corridor: false,
    }));
    wait(app, service, "edge-hover-dismissal", |s| {
        s.presentation.shell == ShellState::Compact && !visible(app, "preview")
    })
    .await?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Shell,
        Request::Activate,
    )));
    settle_explicit_activation(app, service).await?;
    let edge_details = wait(app, service, "edge-details", |s| {
        s.presentation.detail == DetailState::Popup
            && visible(app, "main")
            && s.dashboard_request
                .is_some_and(|r| matches!(r.page, crate::model::DashboardPage::Overview))
    })
    .await?;
    let edge_request = edge_details.dashboard_request.unwrap().sequence;
    let shell = app
        .get_webview_window("shell")
        .ok_or("edge-shell-missing")?;
    let main = app.get_webview_window("main").ok_or("edge-popup-missing")?;
    let shell_position = shell.outer_position().map_err(|_| "edge-shell-position")?;
    let shell_size = shell.outer_size().map_err(|_| "edge-shell-size")?;
    let popup_position = main.outer_position().map_err(|_| "edge-popup-position")?;
    let popup_size = main.outer_size().map_err(|_| "edge-popup-size")?;
    let side = edge_details.settings.windows.edge_side;
    require(
        if side == crate::model::EdgeSide::Right {
            (popup_position.x + popup_size.width as i32 - shell_position.x).abs() <= 2
        } else {
            (shell_position.x + shell_size.width as i32 - popup_position.x).abs() <= 2
        },
        "edge-popup-opens-beside-widget",
    )?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Shell,
        Request::Activate,
    )));
    settle_explicit_activation(app, service).await?;
    wait(app, service, "edge-repeat-activation", |s| {
        s.presentation.detail == DetailState::Popup
            && s.dashboard_request
                .is_some_and(|r| r.sequence > edge_request)
            && visible(app, "main")
    })
    .await?;
    let repeated = service.snapshot().await?;
    require(
        repeated.presentation.shell == ShellState::Compact
            && repeated.presentation.detail == DetailState::Popup,
        "edge-click-does-not-pin-or-dismiss-details",
    )?;
    service.dispatch(Action::Window(Input::Request(
        Surface::Shell,
        Request::OpenWindow,
    )));
    let promoted = wait(app, service, "edge-double-click-window", |s| {
        s.presentation.detail == DetailState::Window
            && visible(app, "main")
            && s.dashboard_request
                .is_some_and(|r| matches!(r.page, crate::model::DashboardPage::Overview))
    })
    .await?;
    require(
        promoted.dashboard_request.is_some_and(|r| {
            r.sequence > edge_request && matches!(r.page, crate::model::DashboardPage::Overview)
        }),
        "double-click-promotes-overview-window",
    )?;
    service.dispatch(Action::Window(Input::Close));
    mode(service, DisplayMode::CompactHud).await?;
    wait(app, service, "hud-hydration", |s| {
        s.settings.mode == DisplayMode::CompactHud && visible(app, "shell")
    })
    .await?;
    let hud = app.get_webview_window("shell").ok_or("missing-shell")?;
    let physical = hud.outer_size().map_err(|_| "hud-size")?;
    require(
        physical.width > physical.height * 2,
        "hud-strip-proportions",
    )?;
    mode(service, DisplayMode::SystemTray).await?;
    wait(app, service, "tray-mode", |s| {
        s.settings.mode == DisplayMode::SystemTray
            && !visible(app, "shell")
            && !visible(app, "preview")
    })
    .await?;
    require(app.tray_by_id("main").is_some(), "tray-retained")?;
    if let Some(rect) = app.tray_by_id("main").and_then(|t| t.rect().ok().flatten()) {
        let p = rect.position.to_physical::<i32>(1.0);
        let s = rect.size.to_physical::<u32>(1.0);
        service.dispatch(Action::Window(Input::Tray {
            rect: crate::geometry::Rect {
                x: p.x as f64,
                y: p.y as f64,
                width: s.width as f64,
                height: s.height as f64,
            },
            activate: false,
            open_window: false,
            menu: false,
        }));
        let foreground = unsafe { GetForegroundWindow() } as usize;
        let entry = service.snapshot().await?;
        service.dispatch(Action::Window(Input::Pointer {
            generation: entry.presentation.generation,
            source: Surface::Tray,
            trigger: false,
            preview: false,
            corridor: false,
        }));
        let mut hydrated = false;
        for _ in 0..160 {
            let state = service.snapshot().await?;
            service.dispatch(Action::Window(Input::Pointer {
                generation: state.presentation.generation,
                source: Surface::Tray,
                trigger: true,
                preview: false,
                corridor: false,
            }));
            if state.presentation.preview_source == Surface::Tray
                && state.presentation.shell == ShellState::Preview
                && visible(app, "preview")
            {
                hydrated = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        require(hydrated, "tray-preview-hydration")?;
        let preview = app.get_webview_window("preview").ok_or("missing-preview")?;
        let size = preview.outer_size().map_err(|_| "preview-size")?;
        let scale = preview.scale_factor().map_err(|_| "preview-scale")?;
        require(
            (size.width as f64 / scale - 260.0).abs() <= 2.0 && size.height as f64 / scale >= 76.0,
            "tray-preview-size",
        )?;
        require(
            unsafe { GetForegroundWindow() } as usize == foreground,
            "tray-preview-preserves-foreground",
        )?;
    } else {
        println!("native-check tray-geometry=unverified-icon-rectangle-unavailable");
    }
    require(
        app.webview_windows().len() <= 3,
        "single-shell-preview-dashboard",
    )?;
    let (reply, receive) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::Reminder {
            request: crate::reminders::Request::Snooze {
                choice: crate::reminders::SnoozeChoice::FifteenMinutes,
                rule: None,
            },
            reply,
        })
        .await
        .map_err(|_| "service-stopped")?;
    let state = receive.await.map_err(|_| "service-stopped")??;
    let now = crate::model::now_ms();
    require(
        state
            .reminders
            .snoozed_until
            .is_some_and(|until| until > now + 14 * 60_000 && until <= now + 15 * 60_000),
        "reminder-snooze-state",
    )?;
    let directory = app
        .path()
        .app_local_data_dir()
        .map_err(|_| "check-storage-path")?
        .join("native-window-check");
    let (_, stored) = crate::storage::Store::open(&directory);
    require(
        stored.reminders.snoozed_until == state.reminders.snoozed_until
            && stored.settings.revision == state.settings.revision
            && stored.diagnostic.is_none(),
        "reminder-snooze-atomic-persistence",
    )?;
    let (reply, receive) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::Reminder {
            request: crate::reminders::Request::Snooze {
                choice: crate::reminders::SnoozeChoice::Resume,
                rule: None,
            },
            reply,
        })
        .await
        .map_err(|_| "service-stopped")?;
    let state = receive.await.map_err(|_| "service-stopped")??;
    require(
        state.reminders.snoozed_until.is_none(),
        "reminder-resume-state",
    )?;
    service.dispatch(Action::Window(Input::Close));
    service.dispatch(Action::Window(Input::Request(
        Surface::Tray,
        Request::Collapse,
    )));
    wait(app, service, "closed-before-retire", |s| {
        s.presentation.detail == DetailState::Closed && s.presentation.shell == ShellState::Compact
    })
    .await?;
    // Only the idle-view lease clock advances. Quota/reminder clocks remain real.
    service.dispatch(Action::Window(Input::RetireHidden));
    wait(app, service, "all-unused-webviews-retired", |_| {
        app.webview_windows().is_empty()
    })
    .await?;
    require(
        app.tray_by_id("main").is_some(),
        "tray-survives-zero-webviews",
    )?;
    let before = crate::native_windows::post_control_check()?;
    wait(
        app,
        service,
        "native-control-survives-zero-webviews",
        |_| {
            app.webview_windows().is_empty() && crate::native_windows::control_reconciles() > before
        },
    )
    .await?;
    let foreground = unsafe { GetForegroundWindow() } as usize;
    second_launch(true).await?;
    require(
        app.webview_windows().is_empty() && unsafe { GetForegroundWindow() } as usize == foreground,
        "duplicate-login-stays-hidden",
    )?;
    second_launch(false).await?;
    wait(app, service, "duplicate-manual-opens-dashboard", |s| {
        s.dashboard_visible && visible(app, "main")
    })
    .await?;
    require(
        app.webview_windows().len() == 1,
        "duplicate-manual-rehydrates-one-dashboard",
    )?;
    let request_before = service
        .snapshot()
        .await?
        .dashboard_request
        .map(|request| request.sequence)
        .unwrap_or(0);
    service.dispatch(Action::Window(Input::Open {
        standard: true,
        settings: true,
    }));
    wait(app, service, "dashboard-recreated", |s| {
        visible(app, "main")
            && s.dashboard_request.is_some_and(|request| {
                request.sequence > request_before
                    && matches!(request.page, crate::model::DashboardPage::Settings)
            })
    })
    .await?;
    require(
        app.webview_windows().len() == 1,
        "lazy-recreated-dashboard-only",
    )?;
    let main = app.get_webview_window("main").ok_or("missing-main")?;
    main.minimize().map_err(|_| "native-minimize")?;
    wait(app, service, "minimized-background-cadence", |s| {
        !s.dashboard_visible && s.effective_refresh_seconds >= s.settings.background_seconds
    })
    .await?;
    service.dispatch(Action::Window(Input::Open {
        standard: true,
        settings: false,
    }));
    wait(app, service, "explicit-restore-after-minimize", |s| {
        s.dashboard_visible
            && visible(app, "main")
            && s.dashboard_request.is_some_and(|request| {
                request.sequence > request_before
                    && matches!(request.page, crate::model::DashboardPage::Overview)
            })
    })
    .await?;
    Ok(())
}
