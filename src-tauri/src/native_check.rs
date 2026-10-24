//! Debug-build integration check. Exercises this application's coordinator, never
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

pub fn requested() -> bool {
    std::env::args().any(|arg| {
        matches!(
            arg.as_str(),
            "--check-native-windows"
                | "--check-native-stress"
                | "--check-native-notifications"
                | "--check-native-sizing"
                | "--check-native-topmost"
        )
    })
}

pub fn sizing_requested() -> bool {
    std::env::args().any(|arg| arg == "--check-native-sizing")
}

pub fn topmost_requested() -> bool {
    std::env::args().any(|arg| arg == "--check-native-topmost")
}

pub fn notifications_requested() -> bool {
    std::env::args().any(|arg| arg == "--check-native-notifications")
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
        println!("native-check {label}=pass");
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
    tauri::async_runtime::spawn(async move {
        let mut result = if topmost_requested() {
            reveal_check(&app, &service).await
        } else if sizing_requested() {
            compact_sizing(&app, &service).await
        } else {
            run(&app, &service).await
        };
        if result.is_ok() && std::env::args().any(|arg| arg == "--check-native-stress") {
            result = stress(&app, &service).await;
        }
        if result.is_ok() && notifications_requested() {
            result = notification_check(&app, &service).await;
        }
        match result {
            Ok(()) => println!("native-check result=pass"),
            Err(error) => eprintln!("native-check result=failed code={error}"),
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
                            }) + 58.0 * f64::from(count),
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
                        require(
                            unsafe {
                                windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow()
                            } as usize
                                == foreground,
                            "hover-preserves-foreground",
                        )?;
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
    reveal_check(app, service).await?;
    popup_stacking(app, service).await?;
    floating_close_to_tray(app, service).await?;
    activation_fallback(app, service).await?;
    save_settings(service, original).await?;
    require(true, "compact-client-sizing")
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
    let foreground = unsafe { GetForegroundWindow() } as usize;
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
    require(
        unsafe { GetForegroundWindow() } as usize == foreground,
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
