mod compact_sizing;
mod configuration_dialog;
mod diagnostics;
mod display_preview;
mod geometry;
mod interaction;
mod lifecycle;
mod model;
mod native_check;
mod native_windows;
mod notification;
mod platform;
mod protocol;
mod proxy;
mod reminders;
mod scheduler;
mod service;
mod storage;
mod storage_location;
mod view_lifecycle;
mod widget_appearance;
mod windows;

use interaction::Surface;
use service::{Action, Service};
use tauri::{Emitter, Manager};
use windows::{Input, Request};

struct TrayControls {
    menu: tauri::menu::Menu<tauri::Wry>,
    modes: Vec<(model::DisplayMode, tauri::menu::CheckMenuItem<tauri::Wry>)>,
    visibility: tauri::menu::MenuItem<tauri::Wry>,
    reset: tauri::menu::MenuItem<tauri::Wry>,
}

#[tauri::command]
fn diagnostics_status() -> diagnostics::Status {
    diagnostics::status()
}

#[tauri::command]
fn diagnostics_report(
    window: tauri::WebviewWindow,
    events: Vec<diagnostics::FrontendEvent>,
) -> Result<(), &'static str> {
    diagnostics::frontend(window.label(), events)
}

#[tauri::command]
async fn open_diagnostics_folder(window: tauri::WebviewWindow) -> Result<(), &'static str> {
    if window.label() != "main" {
        return Err("diagnostics-source");
    }
    let directory = diagnostics::status().directory;
    if directory.is_empty() {
        return Err("diagnostics-directory");
    }
    tauri::async_runtime::spawn_blocking(move || {
        std::fs::create_dir_all(&directory).map_err(|_| "diagnostics-open")?;
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            let operation: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
            let path: Vec<u16> = std::ffi::OsStr::new(&directory)
                .encode_wide()
                .chain(Some(0))
                .collect();
            let result = unsafe {
                windows_sys::Win32::UI::Shell::ShellExecuteW(
                    std::ptr::null_mut(),
                    operation.as_ptr(),
                    path.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    1,
                )
            };
            if result as isize <= 32 {
                return Err("diagnostics-open");
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Err("diagnostics-platform")
        }
    })
    .await
    .map_err(|_| "diagnostics-open")?
}

#[tauri::command]
async fn get_state(service: tauri::State<'_, Service>) -> Result<model::AppState, String> {
    service.snapshot().await
}

#[tauri::command]
async fn refresh(service: tauri::State<'_, Service>) -> Result<(), String> {
    service
        .0
        .send(Action::Refresh)
        .await
        .map_err(|_| "service-stopped".into())
}

#[tauri::command]
async fn retry_autostart(service: tauri::State<'_, Service>) -> Result<model::AppState, String> {
    let (reply, rx) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::Autostart(reply))
        .await
        .map_err(|_| "service-stopped")?;
    rx.await.map_err(|_| "service-stopped")?
}

#[tauri::command]
async fn reminder_action(
    service: tauri::State<'_, Service>,
    request: reminders::Request,
) -> Result<model::AppState, String> {
    let (reply, rx) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::Reminder { request, reply })
        .await
        .map_err(|_| "service-stopped")?;
    rx.await.map_err(|_| "service-stopped")?
}

#[tauri::command]
async fn begin_display_preview(
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    session: String,
    expected_revision: u64,
) -> Result<model::AppState, String> {
    if window.label() != "main" {
        return Err("preview-owner-window".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::BeginPreview {
            session,
            expected_revision,
            reply: tx,
        })
        .await
        .map_err(|_| "service-stopped")?;
    rx.await.map_err(|_| "service-stopped")?
}
#[tauri::command]
async fn update_display_preview(
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    session: String,
    sequence: u64,
    patch: Option<display_preview::Patch>,
) -> Result<model::AppState, String> {
    if window.label() != "main" {
        return Err("preview-owner-window".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::UpdatePreview {
            session,
            sequence,
            patch,
            reply: tx,
        })
        .await
        .map_err(|_| "service-stopped")?;
    rx.await.map_err(|_| "service-stopped")?
}
#[tauri::command]
async fn end_display_preview(
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    session: String,
) -> Result<model::AppState, String> {
    if window.label() != "main" {
        return Err("preview-owner-window".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::EndPreview {
            session: Some(session),
            reply: Some(tx),
        })
        .await
        .map_err(|_| "service-stopped")?;
    rx.await.map_err(|_| "service-stopped".into())
}

#[tauri::command]
async fn save_settings(
    service: tauri::State<'_, Service>,
    expected_revision: u64,
    settings: model::Settings,
    storage_resolution: Option<storage_location::Resolution>,
    storage_token: Option<String>,
) -> Result<model::AppState, serde_json::Value> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    service
        .0
        .send(Action::Save {
            expected_revision,
            settings: Box::new(settings),
            storage_resolution,
            storage_token,
            reply: tx,
        })
        .await
        .map_err(|_| "service-stopped")?;
    rx.await
        .map_err(|_| "service-stopped")?
        .map_err(|code| serde_json::from_str(&code).unwrap_or(serde_json::Value::String(code)))
}

#[tauri::command]
fn inspect_storage_location(
    store: tauri::State<'_, std::sync::Arc<storage_location::Manager>>,
    location: model::StorageLocation,
) -> Result<storage_location::Inspection, String> {
    store.inspect(&location).map_err(str::to_string)
}

#[tauri::command]
async fn choose_storage_folder(window: tauri::WebviewWindow) -> Result<Option<String>, String> {
    let owner = window.hwnd().map_err(|_| "storage-folder-dialog")?.0 as isize;
    tauri::async_runtime::spawn_blocking(move || configuration_dialog::choose(owner))
        .await
        .map_err(|_| "storage-folder-dialog".to_owned())?
}

#[tauri::command]
fn view_ready(
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    generation: u64,
    page: model::DashboardPage,
    width: f64,
    height: f64,
    fit: bool,
) {
    if window.label() == "main" {
        service.dispatch_reliable(Action::Window(Input::DashboardLayout {
            generation,
            page,
            width,
            height,
            fit,
        }));
    }
}

#[tauri::command]
fn hide_dashboard(app: tauri::AppHandle, service: tauri::State<'_, Service>) {
    if app.tray_by_id("main").is_some() {
        service.dispatch(Action::Window(Input::Close));
    }
}

#[tauri::command]
fn surface_action(
    window: tauri::WebviewWindow,
    service: tauri::State<'_, Service>,
    request: Request,
) -> Result<(), String> {
    if matches!(request, Request::InspectFrontend { .. })
        && !crate::native_check::presentation_requested()
    {
        return Err("validation-only-command".into());
    }
    if crate::native_check::requested() {
        if let Request::Ready {
            generation,
            width,
            height,
        } = &request
        {
            println!(
                "native-check content-ready label={} generation={generation} size={width}x{height}",
                window.label()
            );
        }
    }
    let source = match window.label() {
        "main" => Surface::Main,
        "shell" => Surface::Shell,
        "preview" => Surface::Preview,
        _ => return Err("unknown-surface".into()),
    };
    service
        .0
        .try_send(Action::Window(Input::Request(source, request)))
        .map_err(|_| "window-service-busy".into())
}

#[tauri::command]
fn quit(service: tauri::State<'_, Service>) {
    service.dispatch(Action::Quit);
}

fn show_dashboard(app: &tauri::AppHandle, settings: bool) {
    app.state::<Service>().dispatch(Action::Window(Input::Open {
        standard: true,
        settings,
    }));
}

fn tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::{
        menu::{CheckMenuItem, Menu, MenuItem, Submenu},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    };
    let open = MenuItem::with_id(app, "open", "Open dashboard", true, None::<&str>)?;
    let refresh = MenuItem::with_id(app, "refresh", "Refresh usage", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let modes = [
        (model::DisplayMode::Floating, "Floating"),
        (model::DisplayMode::ScreenEdge, "Screen Edge"),
        (model::DisplayMode::SystemTray, "System Tray"),
        (model::DisplayMode::CompactHud, "Compact HUD"),
    ]
    .into_iter()
    .map(|(mode, label)| {
        Ok((
            mode,
            CheckMenuItem::with_id(
                app,
                format!("mode-{}", mode.key()),
                label,
                true,
                mode == model::DisplayMode::SystemTray,
                None::<&str>,
            )?,
        ))
    })
    .collect::<tauri::Result<Vec<_>>>()?;
    let mode_menu = Submenu::with_id_and_items(
        app,
        "display-mode",
        "Display mode",
        true,
        &[&modes[0].1, &modes[1].1, &modes[2].1, &modes[3].1],
    )?;
    let visibility = MenuItem::with_id(app, "visibility", "Hide widget", false, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset-position", "Reset position", false, None::<&str>)?;
    let snooze15 = MenuItem::with_id(app, "snooze-15", "For 15 minutes", true, None::<&str>)?;
    let snooze60 = MenuItem::with_id(app, "snooze-60", "For 1 hour", true, None::<&str>)?;
    let snooze_tomorrow = MenuItem::with_id(
        app,
        "snooze-tomorrow",
        "Until tomorrow 09:00",
        true,
        None::<&str>,
    )?;
    let snooze = Submenu::with_id_and_items(
        app,
        "snooze",
        "Snooze reminders",
        true,
        &[&snooze15, &snooze60, &snooze_tomorrow],
    )?;
    let resume = MenuItem::with_id(app, "resume", "Resume reminders", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &open,
            &refresh,
            &mode_menu,
            &visibility,
            &reset,
            &snooze,
            &resume,
            &settings,
            &quit,
        ],
    )?;
    app.manage(TrayControls {
        menu: menu.clone(),
        modes,
        visibility,
        reset,
    });
    let last_rect = std::sync::Mutex::new(None::<geometry::Rect>);
    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("bundled icon").clone())
        .tooltip("Codex Usage Widget\nUsage unavailable — starting")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            app.state::<Service>()
                .dispatch(Action::Window(Input::Menu(false)));
            match event.id.as_ref() {
                "open" => show_dashboard(app, false),
                "settings" => show_dashboard(app, true),
                "refresh" => app.state::<Service>().dispatch(Action::Refresh),
                "snooze-15" | "snooze-60" | "snooze-tomorrow" | "resume" => {
                    let choice = match event.id.as_ref() {
                        "snooze-15" => reminders::SnoozeChoice::FifteenMinutes,
                        "snooze-60" => reminders::SnoozeChoice::OneHour,
                        "snooze-tomorrow" => reminders::SnoozeChoice::TomorrowNine,
                        _ => reminders::SnoozeChoice::Resume,
                    };
                    let (reply, _) = tokio::sync::oneshot::channel();
                    app.state::<Service>().dispatch(Action::Reminder {
                        request: reminders::Request::Snooze { choice, rule: None },
                        reply,
                    });
                }
                "visibility" => app
                    .state::<Service>()
                    .dispatch(Action::Window(Input::Request(
                        Surface::Tray,
                        Request::ToggleHidden,
                    ))),
                "reset-position" => {
                    app.state::<Service>()
                        .dispatch(Action::Window(Input::Request(
                            Surface::Tray,
                            Request::ResetPosition,
                        )))
                }
                "mode-floating" => app
                    .state::<Service>()
                    .dispatch(Action::SetMode(model::DisplayMode::Floating)),
                "mode-screen-edge" => app
                    .state::<Service>()
                    .dispatch(Action::SetMode(model::DisplayMode::ScreenEdge)),
                "mode-system-tray" => app
                    .state::<Service>()
                    .dispatch(Action::SetMode(model::DisplayMode::SystemTray)),
                "mode-compact-hud" => app
                    .state::<Service>()
                    .dispatch(Action::SetMode(model::DisplayMode::CompactHud)),
                "quit" => {
                    if let Some(window) = app
                        .get_webview_window("main")
                        .filter(|w| w.is_visible().unwrap_or(false))
                    {
                        let _ = window.emit("request-quit", ());
                    } else {
                        app.state::<Service>().dispatch(Action::Quit);
                    }
                }
                _ => {}
            }
        })
        .on_tray_icon_event(move |tray, event| {
            let is_move = matches!(event, TrayIconEvent::Move { .. });
            let (rect, position, activate, open_window, menu) = match event {
                TrayIconEvent::Enter { rect, position, .. }
                | TrayIconEvent::Move { rect, position, .. } => {
                    (rect, position, false, false, false)
                }
                TrayIconEvent::Click {
                    rect,
                    position,
                    button,
                    button_state,
                    ..
                } => (
                    rect,
                    position,
                    button == MouseButton::Left && button_state == MouseButtonState::Up,
                    false,
                    button == MouseButton::Right && button_state == MouseButtonState::Down,
                ),
                TrayIconEvent::DoubleClick {
                    rect,
                    position,
                    button,
                    ..
                } => (rect, position, false, button == MouseButton::Left, false),
                _ => return,
            };
            let app = tray.app_handle();
            let scale = app
                .monitor_from_point(position.x, position.y)
                .ok()
                .flatten()
                .map_or(1.0, |m| m.scale_factor());
            let position = rect.position.to_physical::<i32>(scale);
            let size = rect.size.to_physical::<u32>(scale);
            let bounds = geometry::Rect {
                x: position.x as f64,
                y: position.y as f64,
                width: size.width as f64,
                height: size.height as f64,
            };
            if let Ok(mut previous) = last_rect.lock() {
                if is_move && *previous == Some(bounds) {
                    return;
                }
                *previous = Some(bounds);
            }
            app.state::<Service>().dispatch(Action::Window(Input::Tray {
                rect: bounds,
                activate,
                open_window,
                menu,
            }));
            if !activate && !open_window && !menu {
                app.state::<Service>().dispatch(Action::Tooltip);
            }
        })
        .build(app)?;
    Ok(())
}

pub fn run() {
    let context = tauri::generate_context!();
    // Coordinator/sizing checks must neither forward into a running user instance
    // nor resolve its configuration. Installed notification checks retain the real identity.
    let context = {
        let mut context = context;
        if native_check::requested() && !native_check::notifications_requested() {
            context.config_mut().identifier.push_str(".validation");
        }
        context
    };
    let app_identity = native_windows::app_identity(&context.config().identifier);
    let launches = std::sync::Arc::new(lifecycle::LaunchRouter::default());
    let forwarded = launches.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            move |_app, args, _cwd| forwarded.forward(&args),
        ))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name(lifecycle::STARTUP_NAME)
                .arg(lifecycle::LOGIN_ARGUMENT)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            reminder_action,
            get_state,
            diagnostics_status,
            diagnostics_report,
            open_diagnostics_folder,
            refresh,
            retry_autostart,
            save_settings,
            begin_display_preview,
            update_display_preview,
            end_display_preview,
            inspect_storage_location,
            choose_storage_folder,
            view_ready,
            hide_dashboard,
            surface_action,
            quit
        ])
        .setup(move |app| {
            lifecycle::verify_single_instance(app.handle()).map_err(std::io::Error::other)?;
            let tray_available = tray(app).is_ok();
            let directory = app.path().app_local_data_dir()?;
            let directory = if native_check::requested() {
                if native_check::presentation_requested() {
                    native_check::validation_directory()
                } else {
                    directory.join(if native_check::topmost_requested() {
                        "native-topmost-check"
                    } else {
                        "native-window-check"
                    })
                }
            } else {
                directory
            };
            let portable_directory = tauri::utils::platform::current_exe()?
                .parent()
                .ok_or_else(|| std::io::Error::other("executable directory unavailable"))?
                .to_path_buf();
            let portable_directory = if native_check::requested() {
                directory.clone()
            } else {
                portable_directory
            };
            let (service, _) = service::start(
                app.handle().clone(),
                directory,
                portable_directory,
                tray_available,
            );
            app.manage(service.clone());
            if let Err(code) = app_identity {
                service.dispatch_reliable(Action::Window(Input::NativeFault(code)));
            }
            launches.attach(service.clone());
            if let Err(code) = native_windows::initialize(service.clone()) {
                service.dispatch_reliable(Action::Window(Input::NativeFault(code)));
            }
            service.dispatch_reliable(Action::Window(Input::Environment));
            if native_check::requested() {
                native_check::start(app.handle().clone(), service.clone());
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Destroyed => {
                diagnostics::event(
                    "webview_destroyed",
                    || serde_json::json!({"source":window.label()}),
                );
                let source = match window.label() {
                    "main" => Some(Surface::Main),
                    "shell" => Some(Surface::Shell),
                    "preview" => Some(Surface::Preview),
                    _ => None,
                };
                if let (Some(source), Some(service)) = (source, window.try_state::<Service>()) {
                    service.dispatch_reliable(Action::Window(Input::Destroyed(source)));
                }
            }
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                if window.app_handle().tray_by_id("main").is_some() {
                    if window.label() == "main" {
                        let _ = window.emit("request-close", ());
                    } else if let Some(service) = window.try_state::<Service>() {
                        service.dispatch(Action::Window(Input::Request(
                            Surface::Preview,
                            Request::Collapse,
                        )));
                    }
                }
            }
            tauri::WindowEvent::ThemeChanged(_) => {
                if let Some(service) = window.try_state::<Service>() {
                    service.dispatch(Action::Appearance);
                }
            }
            tauri::WindowEvent::Focused(false) => {
                if let Some(service) = window.try_state::<Service>() {
                    service.dispatch(Action::Window(Input::FocusLost));
                }
            }
            tauri::WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(service) = window.try_state::<Service>() {
                    service.dispatch(Action::Window(Input::Layout));
                }
            }
            _ => {}
        })
        .build(context)
        .expect("failed to initialize Codex Usage Widget");
    app.run(|_app, event| {
        if let tauri::RunEvent::ExitRequested { api, code, .. } = event {
            if code.is_none() {
                api.prevent_exit();
            }
        }
    });
}
