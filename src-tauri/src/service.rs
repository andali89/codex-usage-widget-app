use crate::{
    interaction::{DetailState, Surface},
    model::{self, AppState, Settings, Usage},
    platform, protocol, reminders,
    scheduler::Scheduler,
    storage_location::{Manager as Store, Resolution},
    windows::{Input, Windows},
};
use std::{path::PathBuf, time::Duration};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, oneshot, watch};

fn close_to_tray_allowed(source: Surface, mode: model::DisplayMode, tray_available: bool) -> bool {
    source == Surface::Shell && mode == model::DisplayMode::Floating && tray_available
}

pub enum Action {
    Snapshot(oneshot::Sender<AppState>),
    #[cfg(debug_assertions)]
    ShellContentSize(oneshot::Sender<Option<(f64, f64)>>),
    #[cfg(debug_assertions)]
    PreviewContentSize(oneshot::Sender<Option<(f64, f64)>>),
    #[cfg(debug_assertions)]
    DashboardSettled(oneshot::Sender<bool>),
    Refresh,
    Autostart(oneshot::Sender<Result<AppState, String>>),
    Save {
        expected_revision: u64,
        settings: Box<Settings>,
        reply: oneshot::Sender<Result<AppState, String>>,
        storage_resolution: Option<Resolution>,
        storage_token: Option<String>,
    },
    Appearance,
    Tooltip,
    Window(Input),
    SetMode(model::DisplayMode),
    Reminder {
        request: reminders::Request,
        reply: oneshot::Sender<Result<AppState, String>>,
    },
    Quit,
}

#[derive(Clone)]
pub struct Service(pub mpsc::Sender<Action>);

impl Service {
    pub async fn snapshot(&self) -> Result<AppState, String> {
        let (tx, rx) = oneshot::channel();
        self.0
            .send(Action::Snapshot(tx))
            .await
            .map_err(|_| "service-stopped")?;
        rx.await.map_err(|_| "service-stopped".into())
    }
    pub fn dispatch(&self, action: Action) {
        let _ = self.0.try_send(action);
    }
    pub fn dispatch_reliable(&self, action: Action) {
        let tx = self.0.clone();
        tauri::async_runtime::spawn(async move {
            let _ = tx.send(action).await;
        });
    }
}

// A drag is one persisted edit: never publish half of a monitor/side change.
fn save_placement(
    settings: &mut Settings,
    update: crate::windows::PlacementUpdate,
    save: impl FnOnce(&Settings) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let mut next = settings.clone();
    next.windows
        .placements
        .insert(next.mode.key().into(), update.placement);
    if let Some(side) = update.edge_side {
        next.windows.edge_side = side;
    }
    next.revision += 1;
    save(&next)?;
    *settings = next;
    Ok(())
}

fn publish(app: &AppHandle, state: &mut AppState) {
    if let Some(store) = app.try_state::<std::sync::Arc<Store>>() {
        state.storage = store.status();
    }
    state.revision += 1;
    if let Some(tray) = app.tray_by_id("main") {
        let summary = tooltip(&state.usage);
        let _ = tray.set_tooltip(if tooltip_enabled(state) {
            Some(summary.as_str())
        } else {
            None
        });
    }
    if let Some(controls) = app.try_state::<crate::TrayControls>() {
        for (mode, item) in &controls.modes {
            let _ = item.set_checked(*mode == state.settings.mode);
        }
        let desktop = state.settings.mode != model::DisplayMode::SystemTray;
        let _ = controls.visibility.set_enabled(desktop);
        let _ = controls.reset.set_enabled(desktop);
        let _ = controls.visibility.set_text(if state.presentation.hidden {
            "Show widget"
        } else {
            "Hide widget"
        });
    }
    let _ = app.emit("widget-state", &*state);
}

fn tooltip(usage: &Usage) -> String {
    if usage.windows.is_empty() {
        return format!(
            "{}\n{}",
            if usage.consecutive_unavailable_reads < 3
                && !["auth-required", "unsupported", "paused"].contains(&usage.connection.as_str())
            {
                "Checking usage…"
            } else {
                "Usage unavailable"
            },
            if usage.freshness != "stale" && usage.ordinary_usage_allowed == Some(false) {
                "Ordinary usage blocked"
            } else {
                &usage.connection
            }
        );
    }
    let stale = usage.freshness != "fresh";
    let values = usage
        .windows
        .iter()
        .enumerate()
        .map(|(index, w)| {
            let value = match w.remaining_percent {
                Some(p) if p > 0.0 && p < 1.0 => "<1% left".into(),
                Some(p) => format!(
                    "{}% left",
                    if p.fract() == 0.0 {
                        format!("{p:.0}")
                    } else {
                        format!("{p:.1}")
                    }
                ),
                None => "unavailable".into(),
            };
            let label = if w.duration_minutes.is_none() {
                if index == 0 {
                    "Primary (period unknown)"
                } else {
                    "Secondary (period unknown)"
                }
            } else {
                &w.label
            };
            format!(
                "{}{label} {value}",
                if !stale && w.last_known {
                    "Last known: "
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>()
        .join(" · ");
    let status = if stale {
        "Out of date".to_string()
    } else if usage.ordinary_usage_allowed == Some(false) {
        "Ordinary usage blocked".into()
    } else if usage.windows.iter().any(|w| w.last_known) {
        "Some data out of date".into()
    } else if let Some(reset) = usage
        .windows
        .iter()
        .filter(|w| w.quality == "valid")
        .filter_map(|w| w.resets_at_ms.map(|n| (n, &w.label)))
        .min_by_key(|(n, _)| *n)
    {
        if reset.0 <= model::now_ms() {
            "Reset due — unconfirmed".into()
        } else {
            let mins = ((reset.0 - model::now_ms()).max(0) as u64).div_ceil(60000);
            format!(
                "Expected reset: {} in {}h {}m",
                reset.1,
                mins / 60,
                mins % 60
            )
        }
    } else {
        "Codex usage".into()
    };
    format!(
        "{}{values}\n{status}",
        if stale { "Last known: " } else { "" }
    )
}

pub fn start(
    app: AppHandle,
    directory: PathBuf,
    portable_directory: PathBuf,
    tray_available: bool,
) -> (Service, bool) {
    let (store, loaded) = Store::open(portable_directory, directory);
    let store = std::sync::Arc::new(store);
    app.manage(store.clone());
    #[cfg(debug_assertions)]
    let loaded = crate::native_check::prepare(loaded);
    let first_launch = loaded.first_launch;
    let dashboard = crate::lifecycle::initial_dashboard(
        first_launch,
        tray_available,
        crate::lifecycle::background_launch(std::env::args()),
    );
    #[cfg(debug_assertions)]
    let dashboard = dashboard && !crate::native_check::sizing_requested();
    let mut persisted_reminders = serde_json::to_vec(&loaded.reminders).unwrap_or_default();
    let mut reminders = reminders::Engine::new(loaded.reminders, loaded.settings.reminders.enabled);
    let needs_salt = reminders.data.account_salt.is_none();
    if needs_salt {
        reminders.data.account_salt = platform::account_salt();
    }
    let mut identity_salt = reminders.data.account_salt.clone();
    let (tx, mut rx) = mpsc::channel(64);
    let service = Service(tx);
    let actor_service = service.clone();
    let appearance = platform::appearance(loaded.settings.theme_preference);
    let foreground_seconds = loaded.settings.foreground_seconds;
    let mut notification = crate::notification::Adapter::new(&app);
    let mut state = AppState {
        process_epoch: format!("{}-{}", model::now_ms(), std::process::id()),
        revision: 0,
        settings: loaded.settings,
        appearance,
        usage: Usage {
            connection: "starting".into(),
            freshness: "empty".into(),
            ..Usage::default()
        },
        storage_diagnostic: loaded.diagnostic,
        storage: store.status(),
        tray_available,
        dashboard_visible: dashboard,
        presentation: crate::interaction::Presentation {
            detail: if dashboard {
                DetailState::Window
            } else {
                DetailState::Closed
            },
            ..Default::default()
        },
        window_diagnostic: None,
        tooltip_fallback: false,
        effective_refresh_seconds: foreground_seconds,
        monitors: monitor_choices(&app),
        reminders: reminders.snapshot(notification.diagnostic()),
        dashboard_request: None,
        first_launch,
    };
    let mut durable_identity = identity_salt.is_some();
    if first_launch || needs_salt {
        match store.save(&state.settings, &reminders.data) {
            Err(code) => {
                durable_identity = false;
                state.storage_diagnostic = Some(code.into());
            }
            Ok(()) => persisted_reminders = serde_json::to_vec(&reminders.data).unwrap_or_default(),
        }
    }
    reconcile_autostart(&app, &mut state, &store, &reminders.data, true);
    tauri::async_runtime::spawn(async move {
        let mut windows = Windows::new(&state.settings, state.dashboard_visible);
        let started = std::time::Instant::now();
        let mut scheduler = Scheduler::new(model::now_ms() as u64);
        let (result_tx, mut results) = mpsc::channel(1);
        let (stop_tx, stop_rx) = watch::channel(false);
        let mut worker: Option<tauri::async_runtime::JoinHandle<()>> = None;
        let mut generation = 0_u64;
        let mut next_reconcile = 60000_u64;
        let mut last_wall = model::now_ms();
        let mut last_monotonic = 0_u64;
        let mut last_reminder_refresh = None::<u64>;
        let mut timezone = platform::timezone_signature();
        loop {
            let now = started.elapsed().as_millis() as u64;
            let wall = model::now_ms();
            reminders.schedules(
                &state.settings.reminders,
                wall,
                platform::local_minute,
                state.revision,
            );
            let mut transaction = reminders.clone();
            let suppression = reminders::Suppression {
                locked: windows.locked,
                fullscreen: windows.foreground_fullscreen,
            };
            let local = platform::local_minute(wall);
            let delivery = transaction
                .retry(
                    &state.settings.reminders,
                    wall,
                    local,
                    suppression,
                    notification.ready(),
                )
                .or_else(|| {
                    transaction.claim(
                        &state.settings.reminders,
                        wall,
                        local,
                        suppression,
                        notification.ready(),
                    )
                });
            let bytes = serde_json::to_vec(&transaction.data).unwrap_or_default();
            if bytes != persisted_reminders {
                match store.save(&state.settings, &transaction.data) {
                    Ok(()) => {
                        persisted_reminders = bytes;
                        reminders = transaction;
                        if let Some(delivery) = delivery {
                            let _ = submit_claimed(
                                &app,
                                &store,
                                &mut state,
                                &mut reminders,
                                &mut persisted_reminders,
                                &mut notification,
                                &delivery,
                            );
                        }
                        state.reminders = reminders.snapshot(notification.diagnostic());
                        publish(&app, &mut state);
                    }
                    Err(code) => {
                        reminders.data.delivery_paused = true;
                        state.storage_diagnostic = Some(code.into());
                        state.reminders = reminders.snapshot(notification.diagnostic());
                        publish(&app, &mut state);
                    }
                }
            }
            if notification.ready()
                && reminders.needs_revalidation(wall)
                && last_reminder_refresh.is_none_or(|at| now.saturating_sub(at) >= 60_000)
            {
                scheduler.reconcile(now);
                last_reminder_refresh = Some(now);
            }
            let mut interval = windows.cadence(&state.settings);
            let was_expanded = windows.expanded_visible();
            if windows.tick(&app, now) {
                coordinate(&app, &mut state, &mut windows, &actor_service, None);
                interval = windows.cadence(&state.settings);
                scheduler.set_cadence(now, interval);
                if !was_expanded
                    && windows.expanded_visible()
                    && wall - state.usage.last_success_at_ms.unwrap_or(0) > 15000
                {
                    scheduler.request(now);
                }
            }
            if model::freshness_deadline(&state.usage, interval).is_some_and(|at| wall >= at) {
                state.usage.freshness = "stale".into();
                reminders.mark_stale();
                publish(&app, &mut state);
            }
            if now >= next_reconcile {
                reconcile_notifications(
                    &app,
                    &mut notification,
                    &mut reminders,
                    &mut scheduler,
                    now,
                );
                state.reminders = reminders.snapshot(notification.diagnostic());
                reconcile_autostart(&app, &mut state, &store, &reminders.data, false);
                let elapsed_wall = wall - last_wall;
                let elapsed_monotonic = now.saturating_sub(last_monotonic) as i64;
                if (elapsed_wall - elapsed_monotonic).abs() > 60000 {
                    scheduler.reconcile_clock(wall, now);
                    reminders.mark_stale();
                    if state.usage.last_success_at_ms.is_some() {
                        state.usage.freshness = "stale".into();
                    }
                    scheduler.request(now);
                }
                let previous_high_contrast = state.appearance.high_contrast;
                state.appearance = platform::appearance(state.settings.theme_preference);
                reconcile_timezone(
                    &mut timezone,
                    &mut state,
                    &mut reminders,
                    &mut scheduler,
                    &mut generation,
                    wall,
                    now,
                );
                let old_cadence = windows.cadence(&state.settings);
                let old_presentation = windows.interaction.state.generation;
                windows.handle(&app, Input::Environment, &state.settings, now);
                if old_cadence != windows.cadence(&state.settings)
                    || old_presentation != windows.interaction.state.generation
                    || previous_high_contrast != state.appearance.high_contrast
                {
                    coordinate(&app, &mut state, &mut windows, &actor_service, None);
                    scheduler.set_cadence(now, windows.cadence(&state.settings));
                }
                last_wall = wall;
                last_monotonic = now;
                next_reconcile = now + 60000;
                publish(&app, &mut state);
            }
            if scheduler.start_if_due(now) {
                state.usage.refresh_in_flight = true;
                state.usage.last_attempt_at_ms = Some(wall);
                let path = state.settings.codex_path.clone();
                let proxy_url = state.settings.proxy_url.clone();
                let epoch = identity_salt
                    .clone()
                    .unwrap_or_else(|| state.process_epoch.clone());
                let tx = result_tx.clone();
                let stop = stop_rx.clone();
                let request_generation = generation;
                worker = Some(tauri::async_runtime::spawn(async move {
                    let outcome =
                        protocol::read_cycle(path.as_deref(), proxy_url.as_deref(), &epoch, stop)
                            .await;
                    let _ = tx.send((request_generation, outcome)).await;
                }));
                publish(&app, &mut state);
            }
            state.usage.next_refresh_at_ms = scheduler
                .next_deadline()
                .map(|n| wall.saturating_add(n.saturating_sub(now) as i64));
            let wait = scheduler
                .next_deadline()
                .unwrap_or(next_reconcile)
                .min(windows.next_deadline().unwrap_or(next_reconcile))
                .min(next_reconcile)
                .min(
                    model::freshness_deadline(&state.usage, interval)
                        .map(|at| now.saturating_add(at.saturating_sub(wall).max(0) as u64))
                        .unwrap_or(next_reconcile),
                )
                .min(now.saturating_add(reminders.next_wake(wall).saturating_sub(wall) as u64))
                .saturating_sub(now)
                .max(1);
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(wait)) => {},
                Some((request_generation, outcome)) = results.recv() => {
                    worker = None;
                    let now = started.elapsed().as_millis() as u64;
                    let at = model::now_ms();
                    state.usage.refresh_in_flight = false;
                    if request_generation != generation {
                        scheduler.finish(now, true, false, interval, true);
                    } else {
                        match outcome {
                            Ok(observation) => {
                                let context_changed = state.usage.account_context_key.as_deref() != Some(&observation.context);
                                if context_changed { generation += 1;reminders.invalidate_context();scheduler.rebind_reset_context(); }
                                let previous = if context_changed { Usage::default() } else { state.usage.clone() };
                                let last_attempt = state.usage.last_attempt_at_ms;
                                state.usage = model::normalize(&observation.raw, at, &previous);
                                state.usage.account_context_key = Some(observation.context);
                                state.usage.connection_generation = generation;
                                state.usage.last_attempt_at_ms = last_attempt;
                                if observation.dirty { reminders.mark_stale();state.usage.freshness = "stale".into(); state.usage.error_code = Some("reconciliation-pending".into()); }
                                if !observation.stable_identity { state.usage.error_code.get_or_insert("session-scoped-identity".into()); }
                                reminders.observe(&state.settings.reminders,&state.usage,observation.stable_identity && durable_identity,state.revision,at);
                                scheduler.finish(now, true, false, interval, observation.dirty);
                                scheduler.update_deadlines(state.usage.windows.iter().filter(|w| observation.stable_identity && !observation.dirty && w.quality=="valid" && !w.last_known).map(|w| {
                                    let binding=reminders::binding(state.usage.account_context_key.as_deref().unwrap_or_default(),&w.key);
                                    let rule=state.settings.reminders.window_rules.get(&binding).unwrap_or(&state.settings.reminders.defaults);
                                    crate::scheduler::ResetObservation {key:w.key.clone(),deadline:w.resets_at_ms,remaining:w.remaining_percent,highest_threshold:rule.thresholds.first().copied().unwrap_or(20)}
                                }),at,now);
                            },
                            Err(error) => {
                                reminders.mark_stale();
                                let unavailable_reads = state.usage.consecutive_unavailable_reads.saturating_add(1);
                                if error.permanent || error.code == "account-changed-during-read" {
                                    reminders.invalidate_context();
                                    generation += 1;
                                    state.usage = Usage { connection_generation: generation, ..Usage::default() };
                                }
                                state.usage.connection = match error.code { "auth-required" => "auth-required", _ if error.permanent => "unsupported", _ => "offline" }.into();
                                state.usage.consecutive_unavailable_reads = unavailable_reads;
                                state.usage.freshness = if state.usage.windows.is_empty() { "empty" } else { "stale" }.into();
                                state.usage.error_code = Some(if error.is_incompatible() { "incompatible-codex-protocol" } else { error.code }.into());
                                state.usage.codex_cli_version = error.codex_cli_version;
                                state.usage.last_attempt_at_ms = Some(at);
                                scheduler.finish(now, false, error.permanent, interval, false);
                                if error.child_exited && scheduler.unexpected_exit(now) {state.usage.connection="paused".into();state.usage.error_code=Some("codex-restart-circuit-open".into());}
                            }
                        }
                    }
                    state.usage.next_refresh_at_ms = scheduler.next_deadline().map(|n| at + n.saturating_sub(now) as i64);
                    state.reminders=reminders.snapshot(notification.diagnostic());
                    publish(&app, &mut state);
                },
                action = rx.recv() => {
                    let Some(action) = action else { break; };
                    let action = match action {
                        Action::Window(Input::Request(source, crate::windows::Request::CloseToTray {})) => {
                            if !close_to_tray_allowed(source, state.settings.mode, state.tray_available) { continue; }
                            Action::SetMode(model::DisplayMode::SystemTray)
                        },
                        other => other,
                    };
                    match action {
                        Action::Snapshot(reply) => { state.storage = store.status(); let _ = reply.send(state.clone()); },
                        #[cfg(debug_assertions)]
                        Action::ShellContentSize(reply) => { let _ = reply.send(windows.shell_content_size()); },
                        #[cfg(debug_assertions)]
                        Action::PreviewContentSize(reply) => { let _ = reply.send(windows.preview_content_size()); },
                        #[cfg(debug_assertions)]
                        Action::DashboardSettled(reply) => { let _ = reply.send(windows.dashboard_settled()); },
                        Action::Refresh => { scheduler.retry(now); },
                        Action::Autostart(reply) => {
                            let mut settings=state.settings.clone();settings.autostart.pending=true;settings.revision+=1;
                            let result=match store.save(&settings,&reminders.data) {
                                Err(code)=>Err(code.into()),
                                Ok(())=>{state.settings=settings;reconcile_autostart(&app,&mut state,&store,&reminders.data,true);publish(&app,&mut state);Ok(state.clone())}
                            };
                            let _=reply.send(result);
                        },
                        Action::Appearance => { reconcile_notifications(&app,&mut notification,&mut reminders,&mut scheduler,now);state.reminders=reminders.snapshot(notification.diagnostic());state.appearance = platform::appearance(state.settings.theme_preference);reconcile_timezone(&mut timezone,&mut state,&mut reminders,&mut scheduler,&mut generation,wall,now); coordinate(&app, &mut state, &mut windows, &actor_service, None); },
                        Action::Tooltip => { if tooltip_enabled(&state) { if let Some(tray) = app.tray_by_id("main") { let _ = tray.set_tooltip(Some(tooltip(&state.usage))); } } },
                        Action::Window(input) => {
                            let reconcile = matches!(input, Input::Reconcile | Input::Lock(false));
                            let render = !matches!(input, Input::Pointer { .. });
                            let explicit_open = matches!(&input, Input::Open { .. } | Input::Tray { activate:true, .. } | Input::Tray { open_window:true, .. } | Input::Request(_,crate::windows::Request::Activate | crate::windows::Request::OpenWindow | crate::windows::Request::Details));
                            let was_expanded = windows.expanded_visible();
                            let before = windows.interaction.state.generation;
                            let (focus, placement) = windows.handle(&app, input, &state.settings, now);
                            if let Some(placement) = placement {
                                match save_placement(&mut state.settings, placement, |settings| store.save(settings, &reminders.data)) {
                                    Ok(()) => state.storage_diagnostic = None,
                                    Err(code) => state.storage_diagnostic = Some(code.into()),
                                }
                            }
                            if render || before != windows.interaction.state.generation { coordinate(&app, &mut state, &mut windows, &actor_service, focus); }
                            scheduler.set_cadence(now, windows.cadence(&state.settings));
                            if reconcile { scheduler.reconcile_clock(wall,now);reminders.mark_stale();generation += 1; state.usage.freshness = if state.usage.windows.is_empty() { "empty" } else { "stale" }.into(); scheduler.request(now); }
                            if (explicit_open || !was_expanded) && windows.expanded_visible() && wall - state.usage.last_success_at_ms.unwrap_or(0) > 15000 { scheduler.request(now); }
                        },
                        Action::SetMode(mode) => {
                            let mut settings = state.settings.clone(); settings.mode = mode; settings.revision += 1;
                            match store.save(&settings,&reminders.data) {
                                Err(code) => { state.storage_diagnostic = Some(code.into()); publish(&app, &mut state); },
                                Ok(()) => { let reselect = state.settings.mode == mode; windows.settings_changed(&state.settings,&settings); if reselect { windows.reveal_mode(mode); } state.settings = settings; coordinate(&app,&mut state,&mut windows,&actor_service,None); scheduler.set_cadence(now, windows.cadence(&state.settings)); }
                            }
                        },
                        Action::Save { expected_revision, settings, reply, storage_resolution, storage_token } => {
                            let mut settings=*settings;
                            settings.autostart=state.settings.autostart.request(settings.autostart.requested);
                            let outcome = if expected_revision != state.settings.revision { Err("settings-conflict".into()) }
                            else if let Err(code) = settings.validate() { Err(code.into()) }
                            else if let Err(code) = settings.reminders.validate_edit(&state.settings.reminders,wall) {Err(code.into())}
                            else {
                                settings.revision = state.settings.revision + 1;
                                let mut next_reminders=reminders.clone();next_reminders.configure(&state.settings.reminders,&settings.reminders,wall,platform::local_minute(wall));
                                if state.settings.codex_path!=settings.codex_path || state.settings.proxy_url!=settings.proxy_url {next_reminders.invalidate_context();}
                                match store.save_selected(&settings,&next_reminders.data,storage_resolution,storage_token.as_deref()) {
                                    Err(code) => { state.storage_diagnostic = Some(code.into()); publish(&app, &mut state); Err(code.into()) },
                                    Ok(loaded_profile) => {
                                        let replaced_profile = loaded_profile.is_some();
                                        if let Some(loaded) = loaded_profile {
                                            settings = loaded.settings;
                                            settings.autostart = state.settings.autostart.request(settings.autostart.requested);
                                            next_reminders = reminders::Engine::new(loaded.reminders, settings.reminders.enabled);
                                            next_reminders.invalidate_context();
                                            if next_reminders.data.account_salt.is_none() { next_reminders.data.account_salt = platform::account_salt(); }
                                            identity_salt = next_reminders.data.account_salt.clone();
                                            durable_identity = identity_salt.is_some() && store.save(&settings,&next_reminders.data).is_ok();
                                        }
                                        if state.settings.reminders!=settings.reminders {scheduler.reconcile(now);}
                                        reminders=next_reminders;persisted_reminders=serde_json::to_vec(&reminders.data).unwrap_or_default();state.reminders=reminders.snapshot(notification.diagnostic());
                                        if replaced_profile || state.settings.codex_path != settings.codex_path || state.settings.proxy_url != settings.proxy_url {
                                            generation += 1;
                                            scheduler.rebind_reset_context();
                                            state.usage = Usage { connection_generation: generation, connection: "starting".into(), freshness: "empty".into(), refresh_in_flight: scheduler.in_flight, ..Usage::default() };
                                            scheduler.retry(now);
                                        }
                                        windows.settings_changed(&state.settings, &settings);
                                        state.settings = settings;
                                        state.storage_diagnostic = None;
                                        reconcile_autostart(&app,&mut state,&store,&reminders.data,true);
                                        state.appearance = platform::appearance(state.settings.theme_preference);
                                        windows.handle(&app, Input::Environment, &state.settings, now);
                                        coordinate(&app, &mut state, &mut windows, &actor_service, None);
                                        scheduler.set_cadence(now, windows.cadence(&state.settings));
                                        state.appearance = platform::appearance(state.settings.theme_preference);
                                        if let Some(window) = app.get_webview_window("main") {
                                            let theme = match state.settings.theme_preference { model::ThemePreference::System => None, model::ThemePreference::Light => Some(tauri::Theme::Light), model::ThemePreference::Dark => Some(tauri::Theme::Dark) };
                                            let _ = window.set_theme(theme);
                                        }
                                        publish(&app, &mut state);
                                        Ok(state.clone())
                                    }
                                }
                            };
                            let _ = reply.send(outcome);
                        },
                        Action::Reminder {request,reply}=>{
                            if matches!(request,reminders::Request::Test) {reconcile_notifications(&app,&mut notification,&mut reminders,&mut scheduler,now);}
                            let mut next=reminders.clone();
                            let mut test_delivery=None;
                            let outcome=(||->Result<(),String>{
                                match request {
                                    reminders::Request::ClearHistory=>next.data.clear_history(),
                                    reminders::Request::Test=>{
                                        if !notification.ready() {return Err(notification.diagnostic().unwrap_or("notifications-status-unknown".into()));}
                                        test_delivery=Some(next.test_claim(wall,state.revision).map_err(str::to_owned)?);
                                    },
                                    reminders::Request::Snooze {choice,rule}=>{
                                        if rule.as_ref().is_some_and(|key|key.len()>512 || !next.data.pending.contains_key(key) && !next.data.history.iter().any(|d|d.events.iter().any(|c|&c.rule_key==key))) {return Err("unknown-reminder-rule".into());}
                                        let until=match choice {
                                            reminders::SnoozeChoice::FifteenMinutes=>Some(wall+15*60_000),
                                            reminders::SnoozeChoice::OneHour=>Some(wall+60*60_000),
                                            reminders::SnoozeChoice::TomorrowNine=>Some(platform::tomorrow_nine(wall).ok_or("local-time-unavailable")?),
                                            reminders::SnoozeChoice::Resume=>{scheduler.request(now);None},
                                        };
                                        next.snooze(rule,until);
                                    }
                                }
                                store.save(&state.settings,&next.data).map_err(str::to_string)
                            })();
                            let result=match outcome {
                                Ok(())=>{
                                    reminders=next;persisted_reminders=serde_json::to_vec(&reminders.data).unwrap_or_default();
                                    let result=if let Some(delivery)=test_delivery {submit_claimed(&app,&store,&mut state,&mut reminders,&mut persisted_reminders,&mut notification,&delivery)}else{Ok(())};
                                    state.reminders=reminders.snapshot(notification.diagnostic());publish(&app,&mut state);result.map(|()|state.clone())
                                },
                                Err(code)=>{state.reminders=reminders.snapshot(Some(code.clone()));publish(&app,&mut state);Err(code)},
                            };
                            let _=reply.send(result);
                        },
                        Action::Quit => {
                            let _ = app.run_on_main_thread(crate::native_windows::shutdown);
                            let _ = stop_tx.send(true);
                            if let Some(mut task) = worker.take() {
                                if tokio::time::timeout(Duration::from_millis(2200), &mut task).await.is_err() { task.abort(); }
                            }
                            app.exit(0);
                            break;
                        }
                    }
                }
            }
        }
    });
    (service, first_launch)
}

fn reconcile_timezone(
    previous: &mut Option<Vec<i64>>,
    state: &mut AppState,
    reminders: &mut reminders::Engine,
    scheduler: &mut Scheduler,
    generation: &mut u64,
    wall: i64,
    now: u64,
) {
    if timezone_changed(previous, platform::timezone_signature()) {
        *generation += 1;
        reminders.mark_stale();
        state.usage.freshness = if state.usage.windows.is_empty() {
            "empty"
        } else {
            "stale"
        }
        .into();
        scheduler.reconcile_clock(wall, now);
        scheduler.reconcile(now);
    }
}
fn timezone_changed(previous: &mut Option<Vec<i64>>, current: Option<Vec<i64>>) -> bool {
    let Some(current) = current else {
        return false;
    };
    let changed = previous.as_ref().is_some_and(|old| *old != current);
    *previous = Some(current);
    changed
}

fn reconcile_notifications(
    app: &AppHandle,
    notification: &mut crate::notification::Adapter,
    reminders: &mut reminders::Engine,
    scheduler: &mut Scheduler,
    now: u64,
) {
    let was_ready = notification.ready();
    notification.reconcile(app);
    if !was_ready && notification.ready() {
        reminders.mark_stale();
        scheduler.reconcile(now);
    }
}

fn submit_claimed(
    app: &AppHandle,
    store: &Store,
    state: &mut AppState,
    reminders: &mut reminders::Engine,
    persisted: &mut Vec<u8>,
    notification: &mut crate::notification::Adapter,
    delivery: &reminders::Delivery,
) -> Result<(), String> {
    // Every caller has already saved the claim. This boundary never queues first.
    let outcome = notification.submit(
        app,
        delivery,
        &state.settings.reminders,
        &state.usage,
        model::now_ms(),
    );
    let failed = matches!(outcome, reminders::Outcome::DefinitelyNotSubmitted);
    reminders.finish(delivery.id, outcome, model::now_ms());
    if let Err(code) = store.save(&state.settings, &reminders.data) {
        reminders.data.delivery_paused = true;
        state.storage_diagnostic = Some(code.into());
        return Err(code.into());
    }
    *persisted = serde_json::to_vec(&reminders.data).unwrap_or_default();
    if failed {
        Err(notification
            .diagnostic()
            .unwrap_or("notifications-preparation-failed".into()))
    } else {
        Ok(())
    }
}

fn reconcile_autostart(
    app: &AppHandle,
    state: &mut AppState,
    store: &Store,
    ledger: &reminders::Durable,
    apply: bool,
) {
    let next = crate::lifecycle::autostart(app, &state.settings.autostart, apply);
    if next == state.settings.autostart {
        return;
    }
    let had_pending_intent = state.settings.autostart.pending;
    let mut settings = state.settings.clone();
    settings.autostart = next;
    settings.revision += 1;
    if let Err(code) = store.save(&settings, ledger) {
        // The previously committed pending intent survives restart. Keep the observed
        // result visible, but do not report completion of its persistence boundary.
        settings.autostart.pending |= apply && had_pending_intent;
        state.storage_diagnostic = Some(code.into());
    }
    state.settings = settings;
}

fn coordinate(
    app: &AppHandle,
    state: &mut AppState,
    windows: &mut Windows,
    service: &Service,
    focus: Option<Surface>,
) {
    if let Err(code) = windows.synchronize(
        app,
        &state.settings,
        service,
        focus,
        !state.appearance.high_contrast,
    ) {
        windows.coordination_fault = Some(code.into());
        if windows.interaction.state.detail == DetailState::Popup && focus == Some(Surface::Main) {
            windows.interaction.open_detail(true);
            let _ = windows.synchronize(
                app,
                &state.settings,
                service,
                None,
                !state.appearance.high_contrast,
            );
        } else if windows.interaction.state.shell == crate::interaction::ShellState::Preview {
            windows.interaction.collapse();
            if let Some(preview) = app.get_webview_window("preview") {
                crate::native_windows::hide(&preview);
            }
        }
    } else {
        windows.coordination_fault = None;
    }
    state.presentation = windows.interaction.state.clone();
    state.dashboard_visible =
        state.presentation.detail != DetailState::Closed && !windows.locked && !windows.minimized;
    state.dashboard_request = windows.dashboard_request;
    state.window_diagnostic = windows
        .coordination_fault
        .clone()
        .or_else(|| windows.diagnostic.clone());
    state.tooltip_fallback = windows.tooltip_fallback;
    state.effective_refresh_seconds = windows.cadence(&state.settings) / 1000;
    state.monitors = monitor_choices(app);
    publish(app, state);
}

fn monitor_choices(app: &AppHandle) -> Vec<model::MonitorChoice> {
    app.available_monitors()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|m| {
            Some(model::MonitorChoice {
                name: m.name()?.clone(),
                width: m.size().width,
                height: m.size().height,
                scale: m.scale_factor(),
            })
        })
        .collect()
}

fn tooltip_enabled(state: &AppState) -> bool {
    state.tooltip_fallback
        && state.settings.windows.hover_enabled
        && state.presentation.detail == DetailState::Closed
        && !state.presentation.menu_open
        && !state.presentation.tray_suppressed
}

#[cfg(test)]
mod tests {
    #[test]
    fn edge_drop_persists_monitor_side_and_height_together_or_preserves_previous_settings() {
        let mut settings = crate::model::Settings {
            mode: crate::model::DisplayMode::ScreenEdge,
            ..Default::default()
        };
        let before = serde_json::to_value(&settings).unwrap();
        let update = || crate::windows::PlacementUpdate {
            placement: crate::geometry::Placement {
                monitor: Some("drop-monitor".into()),
                x: 8.0,
                y: 120.5,
            },
            edge_side: Some(crate::model::EdgeSide::Left),
        };
        assert_eq!(
            super::save_placement(&mut settings, update(), |_| Err("storage-commit")),
            Err("storage-commit")
        );
        assert_eq!(serde_json::to_value(&settings).unwrap(), before);
        let revision = settings.revision;
        super::save_placement(&mut settings, update(), |next| {
            let restored: crate::model::Settings =
                serde_json::from_value(serde_json::to_value(next).unwrap()).unwrap();
            let placement = &restored.windows.placements["screen-edge"];
            assert_eq!(placement.monitor.as_deref(), Some("drop-monitor"));
            assert_eq!(placement.y, 120.5);
            assert_eq!(restored.windows.edge_side, crate::model::EdgeSide::Left);
            assert_eq!(restored.revision, revision + 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(settings.revision, revision + 1);
    }

    #[test]
    fn floating_close_requires_its_shell_and_a_recoverable_tray() {
        use super::*;
        assert!(close_to_tray_allowed(
            Surface::Shell,
            model::DisplayMode::Floating,
            true
        ));
        assert!(!close_to_tray_allowed(
            Surface::Shell,
            model::DisplayMode::Floating,
            false
        ));
        for source in [Surface::Main, Surface::Preview, Surface::Tray] {
            assert!(!close_to_tray_allowed(
                source,
                model::DisplayMode::Floating,
                true
            ));
        }
        for mode in [
            model::DisplayMode::SystemTray,
            model::DisplayMode::ScreenEdge,
            model::DisplayMode::CompactHud,
        ] {
            assert!(!close_to_tray_allowed(Surface::Shell, mode, true));
        }
        assert!(
            serde_json::from_str::<crate::windows::Request>(r#"{"type":"close-to-tray"}"#).is_ok()
        );
        assert!(serde_json::from_str::<crate::windows::Request>(
            r#"{"type":"close-to-tray","mode":"floating"}"#
        )
        .is_err());
    }
    use super::*;
    #[test]
    fn appearance_events_only_reconcile_usage_when_timezone_configuration_changes() {
        let mut previous = Some(vec![1, 2, 3]);
        assert!(!timezone_changed(&mut previous, Some(vec![1, 2, 3])));
        assert!(!timezone_changed(&mut previous, None));
        assert!(timezone_changed(&mut previous, Some(vec![1, 2, 4])));
        assert!(!timezone_changed(&mut previous, Some(vec![1, 2, 4])));
        let mut unknown = None;
        assert!(!timezone_changed(&mut unknown, Some(vec![1, 2, 3])));
    }
    use serde_json::json;
    #[test]
    fn tooltip_gives_status_priority_and_never_turns_unknown_into_zero() {
        let mut u = model::normalize(
            &json!({"rateLimits":{"primary":{"usedPercent":32,"windowDurationMins":300,"resetsAt":1}},"ordinaryUsageAllowed":false}),
            1,
            &Usage::default(),
        );
        assert!(tooltip(&u).contains("Ordinary usage blocked"));
        assert!(tooltip(&u).contains("unavailable"));
        u.freshness = "stale".into();
        assert!(tooltip(&u).starts_with("Last known:"));
        assert!(tooltip(&u).contains("Out of date"));
        assert!(!tooltip(&u).contains("Next expected"));
        let empty = model::normalize(&json!({"ordinaryUsageAllowed":false}), 1, &Usage::default());
        assert!(tooltip(&empty).contains("Ordinary usage blocked"));
    }
}
