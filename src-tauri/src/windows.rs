use crate::{
    geometry::{self, Placement, Rect},
    interaction::{ClickActivation, DetailState, Interaction, ShellState, Surface},
    model::{DashboardPage, DashboardRequest, DisplayMode, EdgeSide, Settings},
    native_windows as native,
    service::{Action, Service},
    view_lifecycle::Lease,
};
use serde::Deserialize;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Request {
    // CSS/logical client-area dimensions, not native outer window bounds.
    Ready {
        generation: u64,
        width: f64,
        height: f64,
    },
    Activate,
    OpenWindow,
    ContextMenu {
        x: f64,
        y: f64,
    },
    Details,
    Settings {},
    CloseToTray {},
    Promote,
    Collapse,
    Escape,
    Drag,
    InspectFrontend {
        message: String,
    },
    ToggleHidden,
    ResetPosition,
    Nudge {
        dx: i16,
        dy: i16,
    },
}
pub enum Input {
    Request(Surface, Request),
    Pointer {
        generation: u64,
        source: Surface,
        trigger: bool,
        preview: bool,
        corridor: bool,
    },
    Tray {
        rect: Rect,
        activate: bool,
        open_window: bool,
        menu: bool,
    },
    Open {
        standard: bool,
        settings: bool,
    },
    Close,
    FocusLost,
    Outside,
    Menu(bool),
    DragStarted,
    DragFinished {
        window: usize,
        bounds: Option<Rect>,
        pointer: Option<(f64, f64)>,
    },
    Layout,
    Environment,
    ExternalForeground,
    Lock(bool),
    Reconcile,
    NativeFault(&'static str),
    DashboardLayout {
        generation: u64,
        page: DashboardPage,
        width: f64,
        height: f64,
        fit: bool,
    },
    MainUserSizing,
    Minimized(bool),
    Destroyed(Surface),
    RetireHidden,
    InspectPopup,
    InspectActivationDenied,
    InspectDragStart,
}

pub struct PlacementUpdate {
    pub placement: Placement,
    pub edge_side: Option<EdgeSide>,
    pub anchors: Option<geometry::SnapAnchors>,
}
impl From<Placement> for PlacementUpdate {
    fn from(placement: Placement) -> Self {
        Self {
            placement,
            edge_side: None,
            anchors: None,
        }
    }
}
#[derive(Clone, Copy)]
struct DragSession {
    window: usize,
    mode: DisplayMode,
    start: Rect,
}
impl DragSession {
    fn accepts(self, window: usize, mode: DisplayMode, locked: bool, bounds: Rect) -> bool {
        self.window == window
            && self.mode == mode
            && !locked
            && (self.start.x != bounds.x || self.start.y != bounds.y)
    }
}

pub struct Windows {
    preview_freeze: bool,
    pub gap_constraints: geometry::GapConstraints,
    inferred_anchors: std::collections::BTreeMap<String, geometry::SnapAnchors>,
    pub composition_failed: bool,
    pub interaction: Interaction,
    pub diagnostic: Option<String>,
    pub coordination_fault: Option<String>,
    pub tooltip_fallback: bool,
    pub battery_saver: bool,
    pub locked: bool,
    tray: Option<Rect>,
    ready_main: bool,
    ready_shell: Option<u64>,
    ready_preview: Option<u64>,
    shell_size: (f64, f64),
    preview_size: (f64, f64),
    pub trigger: Option<Rect>,
    focus_due: Option<u64>,
    last_detail: DetailState,
    pending_focus: Option<Surface>,
    pending_raise: Option<DisplayMode>,
    detail_anchor: Surface,
    idle: [Lease; 3],
    now: u64,
    pub minimized: bool,
    pub foreground_fullscreen: bool,
    dashboard_sequence: u64,
    pub dashboard_request: Option<DashboardRequest>,
    drag_session: Option<DragSession>,
    clicks: ClickActivation,
    attention_pending: bool,
    main_content: Option<(f64, f64)>,
    main_page: DashboardPage,
    layout_dirty: bool,
    fit_after_transition: bool,
    main_user_sized: bool,
    main_sizes: [Option<(f64, f64)>; 2],
    main_reclamp: bool,
    overview_heights: [Option<f64>; 2],
}

impl Windows {
    fn open_widget_settings(&mut self, source: Surface) -> Option<Surface> {
        if !matches!(source, Surface::Shell | Surface::Preview) {
            return None;
        }
        self.clicks.cancel();
        self.detail_anchor = if source == Surface::Shell {
            Surface::Shell
        } else {
            self.interaction.state.preview_source
        };
        self.interaction.open_detail(false);
        self.request_dashboard(DashboardPage::Settings);
        Some(Surface::Main)
    }
    fn request_dashboard(&mut self, page: DashboardPage) {
        self.dashboard_sequence += 1;
        self.dashboard_request = Some(DashboardRequest {
            sequence: self.dashboard_sequence,
            page,
        });
        self.ready_main = false;
    }

    pub fn new(settings: &Settings, dashboard: bool) -> Self {
        Self {
            preview_freeze: false,
            gap_constraints: geometry::GapConstraints::default(),
            inferred_anchors: std::collections::BTreeMap::new(),
            composition_failed: false,
            interaction: Interaction::new(
                settings.mode,
                if dashboard {
                    DetailState::Window
                } else {
                    DetailState::Closed
                },
            ),
            diagnostic: None,
            coordination_fault: None,
            tooltip_fallback: false,
            battery_saver: native::battery_saver(),
            locked: false,
            tray: None,
            ready_main: false,
            ready_shell: None,
            ready_preview: None,
            shell_size: match settings.mode {
                DisplayMode::ScreenEdge => (64.0, 96.0),
                DisplayMode::CompactHud => (440.0, 44.0),
                _ => (336.0, 72.0),
            },
            preview_size: if settings.mode == DisplayMode::SystemTray {
                (260.0, 76.0)
            } else {
                (340.0, 330.0)
            },
            trigger: None,
            focus_due: None,
            last_detail: DetailState::Closed,
            pending_focus: if dashboard { Some(Surface::Main) } else { None },
            pending_raise: None,
            detail_anchor: Surface::Tray,
            idle: [Lease::default(); 3],
            now: 0,
            minimized: false,
            foreground_fullscreen: native::foreground_fullscreen(),
            dashboard_sequence: 0,
            dashboard_request: None,
            drag_session: None,
            clicks: ClickActivation::default(),
            attention_pending: false,
            main_content: None,
            main_page: DashboardPage::Overview,
            layout_dirty: false,
            fit_after_transition: false,
            main_user_sized: false,
            main_sizes: [None, None],
            main_reclamp: false,
            overview_heights: [None, None],
        }
    }
    pub fn next_deadline(&self) -> Option<u64> {
        self.interaction
            .next_deadline()
            .into_iter()
            .chain(self.focus_due)
            .chain(self.clicks.deadline())
            .chain(self.idle.iter().filter_map(Lease::deadline))
            .min()
    }
    fn dashboard_layout(
        &mut self,
        generation: u64,
        page: DashboardPage,
        width: f64,
        height: f64,
        fit: bool,
    ) {
        if self.interaction.state.detail == DetailState::Closed
            || generation != self.interaction.state.generation
            || !width.is_finite()
            || !height.is_finite()
            || !(280.0..=2000.0).contains(&width)
            || !(100.0..=20000.0).contains(&height)
        {
            return;
        }
        let preferred = if self.interaction.state.detail == DetailState::Window {
            620.0
        } else {
            480.0
        };
        let index = usize::from(self.interaction.state.detail == DetailState::Window);
        let initial = self.main_sizes[index].is_none();
        let correct_width =
            (width - self.main_sizes[index].map_or(preferred, |s| s.0)).abs() <= 2.0;
        if correct_width && !(index == 1 && self.main_user_sized) {
            let width_changed = self
                .main_content
                .is_some_and(|(previous, _)| (previous - width).abs() > 1.0);
            if page == DashboardPage::Overview
                && (fit || initial || self.fit_after_transition || width_changed)
            {
                let changed =
                    self.overview_heights[index].is_none_or(|old| (old - height).abs() >= 1.0);
                self.overview_heights[index] = Some(height);
                if changed || initial || self.fit_after_transition || self.main_page != page {
                    self.main_sizes[index] = Some((width, height));
                    self.layout_dirty = true;
                }
            } else if initial {
                self.main_sizes[index] = Some((preferred, if index == 1 { 640.0 } else { 600.0 }));
                self.layout_dirty = true;
            }
            self.fit_after_transition = false;
        }
        self.main_content = Some((width, height));
        self.main_page = page;
        // A cached size can switch presentations before React acknowledges the
        // new width. Do not reveal/settle on the old presentation's measurement.
        self.ready_main = correct_width;
    }
    pub fn tick(&mut self, app: &AppHandle, now: u64) -> bool {
        self.now = now;
        self.retire_idle(app, now);
        let mut changed = self.interaction.tick(now);
        if let Some(source) = self.clicks.take_due(now) {
            self.detail_anchor = source;
            if source == Surface::Tray && self.tray.is_none() {
                self.interaction.open_detail(true);
            } else {
                self.interaction.activate(source);
            }
            self.focus_due = None;
            self.request_dashboard(DashboardPage::Overview);
            self.pending_focus = Some(Surface::Main);
            // Re-anchor an already open popup when another trigger is clicked.
            if self.interaction.state.detail == DetailState::Popup {
                self.last_detail = DetailState::Closed;
            }
            changed = true;
        }
        if self.focus_due.is_some_and(|at| now >= at) {
            self.focus_due = None;
            if !native::owned_focus(app) {
                return self.interaction.outside() || changed;
            }
        }
        changed
    }
    fn retire_idle(&mut self, app: &AppHandle, now: u64) {
        for (index, label) in ["main", "shell", "preview"].into_iter().enumerate() {
            if self.idle[index].take_due(now) {
                if let Some(window) = app.get_webview_window(label) {
                    match window.destroy() {
                        Ok(()) => match index {
                            0 => {
                                self.ready_main = false;
                                self.last_detail = DetailState::Closed;
                            }
                            1 => self.ready_shell = None,
                            _ => self.ready_preview = None,
                        },
                        Err(_) => {
                            self.idle[index].failed(self.now);
                            self.diagnostic = Some("native-window-retire-failed".into());
                        }
                    }
                } else {
                    self.idle[index].destroyed();
                }
            }
        }
    }
    pub fn expanded_visible(&self) -> bool {
        !self.locked
            && if self.minimized {
                self.interaction.preview_visible()
            } else {
                self.interaction.expanded_visible()
            }
    }
    pub fn shell_content_size(&self) -> Option<(f64, f64)> {
        (self.ready_shell == Some(self.interaction.state.generation)).then_some(self.shell_size)
    }
    pub fn preview_content_size(&self) -> Option<(f64, f64)> {
        (self.ready_preview == Some(self.interaction.state.generation)).then_some(self.preview_size)
    }
    pub fn dashboard_settled(&self) -> bool {
        self.ready_main
            && self.pending_focus.is_none()
            && self.clicks.deadline().is_none()
            && !self.fit_after_transition
            && !self.layout_dirty
            && self.last_detail == self.interaction.state.detail
            && self.interaction.state.detail != DetailState::Closed
    }
    pub fn reveal_mode(&mut self, mode: DisplayMode) {
        self.pending_raise = None;
        if mode != DisplayMode::SystemTray {
            self.clicks.cancel();
            self.interaction.hidden(false);
            self.pending_raise = Some(mode);
        }
    }
    fn toggle_hidden(&mut self, mode: DisplayMode) {
        if self.interaction.state.hidden {
            self.reveal_mode(mode);
        } else {
            self.pending_raise = None;
            self.interaction.hidden(true);
        }
    }
    fn take_raise(&mut self, mode: DisplayMode) -> bool {
        if !self.interaction.compact_visible() || self.pending_raise != Some(mode) {
            self.pending_raise = None;
            return false;
        }
        if self.ready_shell == Some(self.interaction.state.generation) {
            self.pending_raise = None;
            return true;
        }
        false
    }
    fn external_foreground(&mut self, settings: &Settings) {
        if settings.windows.always_on_top
            && self.interaction.compact_visible()
            && !self.interaction.state.dragging
        {
            self.pending_raise = Some(settings.mode);
        }
    }
    pub fn settings_changed(&mut self, previous: &Settings, next: &Settings) {
        if previous.windows.always_on_top && !next.windows.always_on_top {
            self.pending_raise = None;
        }
        if previous.mode != next.mode || next.windows.lock_position {
            self.drag_session = None;
            if self.interaction.state.dragging {
                self.interaction.dragging(false);
            }
        }
        self.clicks.cancel();
        if self.interaction.state.detail == DetailState::Popup {
            self.last_detail = DetailState::Closed;
        }
        if !next.windows.hover_enabled && self.interaction.state.shell == ShellState::Preview {
            self.interaction.collapse();
        }
        if previous.mode != next.mode {
            self.interaction.change_mode(next.mode);
            self.ready_shell = None;
            self.ready_preview = None;
            self.shell_size = if next.mode == DisplayMode::ScreenEdge {
                (64.0, 96.0)
            } else if next.mode == DisplayMode::CompactHud {
                (440.0, 44.0)
            } else {
                (336.0, 72.0)
            };
            self.preview_size = if next.mode == DisplayMode::SystemTray {
                (260.0, 76.0)
            } else {
                (340.0, 330.0)
            };
            self.reveal_mode(next.mode);
        }
    }
    pub fn cadence(&self, settings: &Settings) -> u64 {
        let seconds = if self.locked {
            settings.background_seconds
        } else if self.expanded_visible() {
            settings.foreground_seconds
        } else if self.interaction.compact_visible() {
            settings.compact_seconds
        } else {
            settings.background_seconds
        };
        (seconds * if self.battery_saver { 2 } else { 1 }).min(600) * 1000
    }
    pub fn handle(
        &mut self,
        app: &AppHandle,
        input: Input,
        settings: &Settings,
        now: u64,
    ) -> (Option<Surface>, Option<PlacementUpdate>) {
        self.now = now;
        if matches!(
            &input,
            Input::Open { .. }
                | Input::Close
                | Input::Outside
                | Input::Menu(true)
                | Input::DragStarted
                | Input::Lock(true)
                | Input::Layout
                | Input::Destroyed(_)
                | Input::Request(
                    _,
                    Request::Drag
                        | Request::ContextMenu { .. }
                        | Request::Escape
                        | Request::Details
                        | Request::Promote
                        | Request::ToggleHidden
                        | Request::ResetPosition
                )
        ) {
            self.clicks.cancel();
        }
        let mut focus = None;
        let mut placement = None;
        let previous_source = self.interaction.state.preview_source;
        match input {
            Input::DashboardLayout {
                generation,
                page,
                width,
                height,
                fit,
            } => self.dashboard_layout(generation, page, width, height, fit),
            Input::InspectDragStart => {
                self.begin_drag(app, settings);
            }
            Input::MainUserSizing => self.main_user_sized = true,
            Input::Pointer {
                generation,
                source,
                trigger,
                preview,
                corridor,
            } if generation == self.interaction.state.generation => {
                self.interaction.pointer_from(
                    source,
                    trigger,
                    preview,
                    corridor,
                    now,
                    settings.windows.hover_enabled
                        && !(source == Surface::Tray && self.tooltip_fallback),
                );
            }
            Input::Tray {
                rect,
                activate,
                open_window,
                menu,
            } => {
                if rect.width <= 0.0 || rect.height <= 0.0 {
                    self.tooltip_fallback = true;
                    self.diagnostic = Some("tray-geometry-unavailable".into());
                    self.tray = None;
                    self.interaction.collapse();
                    if menu {
                        self.clicks.cancel();
                        self.interaction.menu(true);
                    }
                    if open_window {
                        self.clicks.double(now, native::double_click_interval());
                        self.interaction.open_detail(true);
                        self.request_dashboard(DashboardPage::Overview);
                        focus = Some(Surface::Main);
                    } else if activate {
                        self.clicks
                            .single(now, native::double_click_interval(), Surface::Tray);
                    }
                    return (focus, None);
                }
                self.tray = Some(rect);
                self.environment(app, settings);
                if menu {
                    self.clicks.cancel();
                    self.interaction.menu(true);
                } else if open_window {
                    self.clicks.double(now, native::double_click_interval());
                    self.detail_anchor = Surface::Tray;
                    self.interaction.open_detail(true);
                    self.request_dashboard(DashboardPage::Overview);
                    focus = Some(Surface::Main);
                } else if activate {
                    self.interaction.collapse();
                    self.clicks
                        .single(now, native::double_click_interval(), Surface::Tray);
                }
            }
            Input::Open {
                standard,
                settings: open_settings,
            } => {
                self.interaction.open_detail(standard || open_settings);
                self.request_dashboard(if open_settings {
                    DashboardPage::Settings
                } else {
                    DashboardPage::Overview
                });
                focus = Some(Surface::Main);
            }
            Input::Minimized(value) => {
                self.minimized = value;
                if value && self.pending_focus == Some(Surface::Main) {
                    self.pending_focus = None;
                }
            }
            Input::Destroyed(source) => {
                let index = match source {
                    Surface::Main => {
                        self.ready_main = false;
                        self.last_detail = DetailState::Closed;
                        self.minimized = false;
                        0
                    }
                    Surface::Shell => {
                        self.drag_session = None;
                        if self.interaction.state.dragging {
                            self.interaction.dragging(false);
                        }
                        self.ready_shell = None;
                        1
                    }
                    Surface::Preview => {
                        self.ready_preview = None;
                        2
                    }
                    Surface::Tray => return (None, None),
                };
                self.idle[index].destroyed();
                self.interaction.retire_view();
            }
            Input::RetireHidden => self.retire_idle(app, now + crate::view_lifecycle::IDLE_MS),
            Input::InspectPopup => {
                // Geometry/stacking gate uses the real popup hydration path,
                // independently of Windows granting foreground activation.
                self.detail_anchor = Surface::Shell;
                self.interaction.close_detail();
                self.interaction.open_detail(false);
                self.request_dashboard(
                    self.dashboard_request
                        .map(|r| r.page)
                        .unwrap_or(DashboardPage::Overview),
                );
                self.pending_focus = None;
                self.attention_pending = false;
            }
            Input::InspectActivationDenied => self.activation_denied(),
            Input::Close => {
                self.pending_focus = None;
                self.focus_due = None;
                self.interaction.close_detail();
                self.main_content = None;
                self.attention_pending = false;
            }
            Input::Outside => {
                self.interaction.outside();
            }
            Input::FocusLost => {
                self.focus_due = Some(now + 100);
            }
            Input::Menu(open) => self.interaction.menu(open),
            Input::Lock(locked) => {
                self.locked = locked;
                if locked && self.interaction.state.detail == DetailState::Popup {
                    self.interaction.close_detail();
                }
                self.environment(app, settings);
            }
            Input::Layout => {
                self.main_reclamp = true;
                self.layout_dirty = true;
                if !self.interaction.state.dragging {
                    self.environment(app, settings);
                }
            }
            Input::Environment | Input::Reconcile => {
                if !self.interaction.state.dragging {
                    self.environment(app, settings);
                }
            }
            Input::ExternalForeground => {
                if !self.interaction.state.dragging {
                    self.environment(app, settings);
                    self.external_foreground(settings);
                }
            }
            Input::NativeFault(code) => {
                self.diagnostic = Some(code.into());
                self.tooltip_fallback = true;
                self.interaction.collapse();
            }
            Input::DragStarted => {
                if self.drag_session.is_some() {
                    self.interaction.dragging(true);
                }
            }
            Input::DragFinished {
                window,
                bounds,
                pointer,
            } => {
                if let Some(session) = self.drag_session.filter(|s| s.window == window) {
                    self.drag_session = None;
                    self.interaction.dragging(false);
                    if let Some(bounds) = bounds.filter(|bounds| {
                        session.accepts(
                            window,
                            settings.mode,
                            settings.windows.lock_position || self.locked,
                            *bounds,
                        )
                    }) {
                        placement = self.release_drop(app, settings, bounds, pointer);
                    }
                }
            }
            Input::Request(source, request) => match request {
                Request::InspectFrontend { message }
                    if crate::native_check::presentation_requested() =>
                {
                    crate::native_check::record_frontend_assertion(&message);
                    println!(
                        "native-check frontend={}",
                        message.chars().take(200).collect::<String>()
                    );
                }
                Request::Ready {
                    generation,
                    width,
                    height,
                } => {
                    if crate::native_check::presentation_requested() {
                        println!("native-check ready source={source:?} generation={generation} current={} width={width} height={height}",self.interaction.state.generation);
                    }
                    if width.is_finite()
                        && height.is_finite()
                        && (16.0..=2000.0).contains(&width)
                        && (16.0..=4000.0).contains(&height)
                    {
                        match source {
                            Surface::Main if generation == self.interaction.state.generation => {
                                self.ready_main = true
                            }
                            Surface::Shell if generation == self.interaction.state.generation => {
                                if self.shell_size != (width, height)
                                    && self.detail_anchor == Surface::Shell
                                    && self.interaction.state.detail == DetailState::Popup
                                {
                                    self.last_detail = DetailState::Closed;
                                }
                                self.ready_shell = Some(generation);
                                self.shell_size = (width, height);
                            }
                            Surface::Preview if generation == self.interaction.state.generation => {
                                self.ready_preview = Some(generation);
                                self.preview_size = (width, height);
                            }
                            _ => {}
                        }
                    }
                }
                Request::Activate if source == Surface::Shell => {
                    if !self.interaction.state.dragging {
                        self.interaction.collapse();
                        self.clicks
                            .single(now, native::double_click_interval(), Surface::Shell);
                    }
                }
                Request::OpenWindow if matches!(source, Surface::Shell | Surface::Preview) => {
                    self.clicks.double(now, native::double_click_interval());
                    self.detail_anchor = self.interaction.state.preview_source;
                    self.interaction.open_detail(true);
                    self.request_dashboard(DashboardPage::Overview);
                    focus = Some(Surface::Main);
                }
                Request::ContextMenu { x, y }
                    if matches!(source, Surface::Shell | Surface::Preview)
                        && x.is_finite()
                        && y.is_finite()
                        && x >= 0.0
                        && y >= 0.0 =>
                {
                    let label = if source == Surface::Shell {
                        "shell"
                    } else {
                        "preview"
                    };
                    if let (Some(window), Some(controls)) = (
                        app.get_webview_window(label),
                        app.try_state::<crate::TrayControls>(),
                    ) {
                        let valid_position = window
                            .inner_size()
                            .ok()
                            .zip(window.scale_factor().ok())
                            .is_some_and(|(size, scale)| {
                                scale.is_finite()
                                    && scale > 0.0
                                    && x < size.width as f64 / scale
                                    && y < size.height as f64 / scale
                            });
                        if valid_position {
                            self.interaction.menu(true);
                            if window
                                .popup_menu_at(&controls.menu, tauri::LogicalPosition::new(x, y))
                                .is_err()
                            {
                                self.interaction.menu(false);
                                self.diagnostic = Some("native-context-menu-failed".into());
                            }
                        }
                    }
                }
                Request::Details => {
                    self.detail_anchor = if source == Surface::Shell {
                        Surface::Shell
                    } else {
                        self.interaction.state.preview_source
                    };
                    self.interaction.open_detail(false);
                    self.request_dashboard(DashboardPage::Overview);
                    focus = Some(Surface::Main);
                }
                Request::Settings {} => focus = self.open_widget_settings(source),
                Request::Promote if source == Surface::Main => {
                    self.interaction.open_detail(true);
                    focus = Some(Surface::Main);
                }
                Request::Collapse => self.interaction.collapse(),
                Request::Escape => {
                    self.interaction.escape(source);
                }
                Request::Drag
                    if source == Surface::Shell
                        && !settings.windows.lock_position
                        && self.interaction.compact_visible() =>
                {
                    if let Some(window) = self.begin_drag(app, settings) {
                        if window.start_dragging().is_err() {
                            self.drag_session = None;
                            self.interaction.dragging(false);
                            self.diagnostic = Some("native-drag-failed".into());
                        }
                    }
                }
                Request::ToggleHidden if settings.mode != DisplayMode::SystemTray => {
                    self.toggle_hidden(settings.mode);
                }
                Request::ResetPosition if settings.mode != DisplayMode::SystemTray => {
                    placement = Some(PlacementUpdate {
                        placement: Placement::default(),
                        edge_side: None,
                        anchors: Some(geometry::SnapAnchors {
                            horizontal: if settings.mode == DisplayMode::ScreenEdge {
                                geometry::HorizontalAnchor::None
                            } else {
                                geometry::HorizontalAnchor::Left
                            },
                            vertical: geometry::VerticalAnchor::Top,
                        }),
                    });
                    self.interaction.collapse();
                }
                Request::Nudge { dx, dy }
                    if source == Surface::Shell
                        && !settings.windows.lock_position
                        && (-100..=100).contains(&dx)
                        && (-100..=100).contains(&dy) =>
                {
                    if let Some(mut p) = self.current_placement(app) {
                        p.x += dx as f64;
                        p.y += dy as f64;
                        let mut anchors = self.anchors(settings);
                        if dx != 0 && settings.mode != DisplayMode::ScreenEdge {
                            anchors.horizontal = geometry::HorizontalAnchor::None;
                        }
                        if dy != 0 {
                            anchors.vertical = geometry::VerticalAnchor::None;
                        }
                        placement = Some(PlacementUpdate {
                            placement: p,
                            edge_side: None,
                            anchors: Some(anchors),
                        });
                        self.interaction.collapse();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        if previous_source != self.interaction.state.preview_source {
            self.ready_preview = None;
            self.preview_size = if self.interaction.state.preview_source == Surface::Tray {
                (260.0, 76.0)
            } else {
                (340.0, 330.0)
            };
        }
        if self.interaction.state.detail == DetailState::Closed {
            self.dashboard_request = None;
        }
        (focus, placement)
    }
    fn environment(&mut self, app: &AppHandle, settings: &Settings) {
        self.battery_saver = native::battery_saver();
        self.foreground_fullscreen = native::foreground_fullscreen();
        let foreground = native::foreground_rect();
        let covers = |monitor: Option<tauri::Monitor>| {
            if !settings.windows.suppress_fullscreen {
                return false;
            }
            match &foreground {
                Err(_) => true,
                Ok(None) => false,
                Ok(Some(rect)) => monitor.is_none_or(|m| {
                    rect.x <= m.position().x as f64 + 2.0
                        && rect.y <= m.position().y as f64 + 2.0
                        && rect.right() >= m.position().x as f64 + m.size().width as f64 - 2.0
                        && rect.bottom() >= m.position().y as f64 + m.size().height as f64 - 2.0
                }),
            }
        };
        let fullscreen = covers(self.monitor(app, settings).ok());
        let tray_fullscreen = covers(
            self.tray
                .and_then(|t| app.monitor_from_point(t.x, t.y).ok().flatten())
                .or_else(|| app.primary_monitor().ok().flatten()),
        );
        self.interaction.suppress(self.locked || fullscreen);
        self.interaction
            .suppress_tray(self.locked || tray_fullscreen);
    }
    fn ensure(
        &mut self,
        app: &AppHandle,
        label: &str,
        size: (f64, f64),
        service: &Service,
        topmost: bool,
    ) -> Result<Option<WebviewWindow>, &'static str> {
        let index = match label {
            "main" => 0,
            "shell" => 1,
            _ => 2,
        };
        if self.idle[index].retiring() {
            return Ok(None);
        }
        if let Some(window) = app.get_webview_window(label) {
            return Ok(Some(window));
        }
        let standard = label == "main";
        let diagnostic_script = if crate::native_check::presentation_requested() {
            r#"window.addEventListener('error',e=>window.__TAURI_INTERNALS__.invoke('surface_action',{request:{type:'inspect-frontend',message:String(e.message)}}));window.addEventListener('unhandledrejection',e=>window.__TAURI_INTERNALS__.invoke('surface_action',{request:{type:'inspect-frontend',message:String(e.reason)}}));"#
        } else {
            ""
        };
        let create = |transparent| {
            WebviewWindowBuilder::new(app, label, WebviewUrl::App("index.html".into()))
                .title("Codex Usage Widget")
                .initialization_script(diagnostic_script)
                .visible(false)
                .focused(false)
                .focusable(label != "preview")
                .always_on_top(topmost)
                .decorations(standard)
                .skip_taskbar(!standard)
                .resizable(standard)
                .shadow(standard)
                .transparent(transparent)
                // Keep Tao's DWM transparent-window setup. NOREDIRECTIONBITMAP
                // bypasses it in the pinned Tao version and is unnecessary while
                // this window is hidden until the frontend has hydrated.
                .on_page_load(|window, payload| {
                    if window.label() == "main"
                        && matches!(payload.event(), tauri::webview::PageLoadEvent::Started)
                    {
                        if let Some(service) = window.try_state::<Service>() {
                            service.dispatch_reliable(Action::EndPreview {
                                session: None,
                                reply: None,
                            });
                        }
                    }
                    if crate::native_check::requested() {
                        println!(
                            "native-check page label={} event={:?}",
                            window.label(),
                            payload.event()
                        );
                    }
                })
                .inner_size(size.0, size.1)
                .min_inner_size(if standard { 380.0 } else { 16.0 }, 16.0)
                .build()
        };
        let window = match create(
            label == "shell" && native::composition_available() && !self.composition_failed,
        ) {
            Ok(window) => window,
            Err(_) if label == "shell" => {
                self.composition_failed = true;
                self.diagnostic = Some("native-composition-failed".into());
                create(false).map_err(|_| "native-window-create")?
            }
            Err(_) => return Err("native-window-create"),
        };
        let copy = window.clone();
        let tx = service.clone();
        let surface = if label == "main" {
            Surface::Main
        } else if label == "shell" {
            Surface::Shell
        } else {
            Surface::Preview
        };
        app.run_on_main_thread(move || {
            if let Err(code) = native::install_subclass(&copy, tx.clone(), surface) {
                tx.dispatch(Action::Window(Input::NativeFault(code)));
            }
        })
        .map_err(|_| "native-window-thread")?;
        Ok(Some(window))
    }
    fn monitor(
        &self,
        app: &AppHandle,
        settings: &Settings,
    ) -> Result<tauri::Monitor, &'static str> {
        if settings.mode == DisplayMode::SystemTray {
            if let Some(rect) = self.tray {
                if let Some(monitor) = app.monitor_from_point(rect.x, rect.y).ok().flatten() {
                    return Ok(monitor);
                }
            }
        }
        if let Some(name) = settings
            .windows
            .placements
            .get(settings.mode.key())
            .and_then(|p| p.monitor.as_ref())
        {
            if let Some(monitor) = app
                .available_monitors()
                .ok()
                .and_then(|m| m.into_iter().find(|m| m.name() == Some(name)))
            {
                return Ok(monitor);
            }
        }
        app.primary_monitor()
            .ok()
            .flatten()
            .ok_or("monitor-unavailable")
    }
    fn begin_drag(&mut self, app: &AppHandle, settings: &Settings) -> Option<WebviewWindow> {
        if settings.windows.lock_position || self.locked || !self.interaction.compact_visible() {
            return None;
        }
        let window = app.get_webview_window("shell")?;
        let handle = window.hwnd().ok()?;
        let position = window.outer_position().ok()?;
        let size = window.outer_size().ok()?;
        self.drag_session = Some(DragSession {
            window: handle.0 as usize,
            mode: settings.mode,
            start: Rect {
                x: position.x as f64,
                y: position.y as f64,
                width: size.width as f64,
                height: size.height as f64,
            },
        });
        self.clicks.cancel();
        self.interaction.dragging(true);
        Some(window)
    }
    fn release_drop(
        &self,
        app: &AppHandle,
        settings: &Settings,
        mut bounds: Rect,
        pointer: Option<(f64, f64)>,
    ) -> Option<PlacementUpdate> {
        let point = pointer.unwrap_or((
            bounds.x + bounds.width / 2.0,
            bounds.y + bounds.height / 2.0,
        ));
        let monitor = app.available_monitors().ok()?.into_iter().min_by(|a, b| {
            let distance = |m: &tauri::Monitor| {
                geometry::point_distance(
                    Rect {
                        x: m.position().x as f64,
                        y: m.position().y as f64,
                        width: m.size().width as f64,
                        height: m.size().height as f64,
                    },
                    point,
                )
            };
            distance(a).total_cmp(&distance(b))
        })?;
        let area = monitor.work_area();
        let work = Rect {
            x: area.position.x as f64,
            y: area.position.y as f64,
            width: area.size.width as f64,
            height: area.size.height as f64,
        };
        let scale = monitor.scale_factor();
        // Reconstruct at destination DPI, preserving the center used to choose a side.
        let center = bounds.x + bounds.width / 2.0;
        let shell = app.get_webview_window("shell")?;
        let (width, height) = native::content_outer_size(&shell, self.shell_size, scale).ok()?;
        bounds.x = center - width / 2.0;
        bounds.width = width;
        bounds.height = height;
        let edge = settings.mode == DisplayMode::ScreenEdge;
        let previous_right = settings.windows.edge_side == EdgeSide::Right;
        let delta = bounds.x + bounds.width / 2.0 - work.x - work.width / 2.0;
        let right = if delta == 0.0 {
            previous_right
        } else {
            delta > 0.0
        };
        let (safety, uncertain) = native::taskbar_safety(Rect {
            x: monitor.position().x as f64,
            y: monitor.position().y as f64,
            width: monitor.size().width as f64,
            height: monitor.size().height as f64,
        });
        let constraints = geometry::gap_constraints(
            work,
            (bounds.width, bounds.height),
            scale,
            settings.windows.snap_gap_logical_px,
            safety,
            uncertain,
        );
        let free = geometry::gap_constraints(
            work,
            (bounds.width, bounds.height),
            scale,
            0,
            safety,
            uncertain,
        );
        let anchors = geometry::release_anchors(
            bounds,
            work,
            constraints.margins,
            scale,
            self.anchors(settings),
            edge,
        );
        let rect = geometry::resolve_anchors(
            bounds,
            work,
            anchors,
            constraints.margins,
            free.margins,
            edge.then_some(right),
        );
        Some(PlacementUpdate {
            placement: Placement::from_physical(rect, work, scale, monitor.name().cloned()),
            edge_side: edge.then_some(if right {
                EdgeSide::Right
            } else {
                EdgeSide::Left
            }),
            anchors: Some(anchors),
        })
    }
    pub fn anchors(&self, settings: &Settings) -> geometry::SnapAnchors {
        settings
            .windows
            .snap_anchors
            .get(settings.mode.key())
            .or_else(|| self.inferred_anchors.get(settings.mode.key()))
            .copied()
            .unwrap_or_default()
    }
    pub fn merge_inferred(&self, settings: &mut Settings) {
        for (mode, anchors) in &self.inferred_anchors {
            settings
                .windows
                .snap_anchors
                .entry(mode.clone())
                .or_insert(*anchors);
        }
    }
    pub fn current_placement(&self, app: &AppHandle) -> Option<Placement> {
        let window = app.get_webview_window("shell")?;
        let position = window.outer_position().ok()?;
        let size = window.outer_size().ok()?;
        let monitor = app
            .monitor_from_point(position.x as f64, position.y as f64)
            .ok()
            .flatten()
            .or_else(|| app.primary_monitor().ok().flatten())?;
        let work = monitor.work_area();
        let work = Rect {
            x: work.position.x as f64,
            y: work.position.y as f64,
            width: work.size.width as f64,
            height: work.size.height as f64,
        };
        let rect = Rect {
            x: position.x as f64,
            y: position.y as f64,
            width: size.width as f64,
            height: size.height as f64,
        }
        .clamp(work);
        Some(Placement::from_physical(
            rect,
            work,
            monitor.scale_factor(),
            monitor.name().cloned(),
        ))
    }
    pub fn set_preview_freeze(&mut self, freeze: bool) {
        if self.preview_freeze && !freeze {
            self.layout_dirty = true;
        }
        self.preview_freeze = freeze;
    }
    pub fn synchronize(
        &mut self,
        app: &AppHandle,
        settings: &Settings,
        service: &Service,
        focus: Option<Surface>,
        shadow: bool,
    ) -> Result<(), &'static str> {
        if settings.mode == DisplayMode::SystemTray {
            self.gap_constraints = geometry::GapConstraints::default();
        }
        if focus.is_some() {
            self.pending_focus = focus;
        }
        if self.interaction.state.shell == ShellState::Compact
            && self.pending_focus == Some(Surface::Preview)
        {
            self.pending_focus = None;
        }
        if self.interaction.state.detail == DetailState::Closed
            && self.pending_focus == Some(Surface::Main)
        {
            self.pending_focus = None;
        }
        let monitor = self.monitor(app, settings)?;
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let work = Rect {
            x: area.position.x as f64,
            y: area.position.y as f64,
            width: area.size.width as f64,
            height: area.size.height as f64,
        };
        if !self.interaction.compact_visible() || self.pending_raise != Some(settings.mode) {
            self.pending_raise = None;
        }
        let raise_shell = self.ready_shell == Some(self.interaction.state.generation)
            && self.take_raise(settings.mode);
        let state = self.interaction.state.clone();
        let mut activation_denied = false;
        let mut trigger = if settings.mode == DisplayMode::SystemTray {
            self.tray
        } else {
            None
        };
        if settings.mode != DisplayMode::SystemTray && self.interaction.compact_visible() {
            if let Some(shell) = self.ensure(
                app,
                "shell",
                self.shell_size,
                service,
                settings.windows.always_on_top,
            )? {
                native::configure(
                    &shell,
                    native::WindowPolicy {
                        tool: true,
                        no_activate: false,
                        topmost: settings.windows.always_on_top,
                        shadow: false,
                        standard: None,
                    },
                )?;
                let outer_size = native::content_outer_size(&shell, self.shell_size, scale)?;
                let placement = settings
                    .windows
                    .placements
                    .get(settings.mode.key())
                    .cloned()
                    .unwrap_or_default();
                let monitor_rect = Rect {
                    x: monitor.position().x as f64,
                    y: monitor.position().y as f64,
                    width: monitor.size().width as f64,
                    height: monitor.size().height as f64,
                };
                let (safety, uncertain) = native::taskbar_safety(monitor_rect);
                self.gap_constraints = geometry::gap_constraints(
                    work,
                    outer_size,
                    scale,
                    settings.windows.snap_gap_logical_px,
                    safety,
                    uncertain,
                );
                let free = geometry::gap_constraints(work, outer_size, scale, 0, safety, uncertain);
                if !settings
                    .windows
                    .snap_anchors
                    .contains_key(settings.mode.key())
                    && !self.inferred_anchors.contains_key(settings.mode.key())
                    && self.ready_shell == Some(state.generation)
                {
                    let raw = Rect {
                        x: work.x + placement.x * scale,
                        y: work.y + placement.y * scale,
                        width: outer_size.0,
                        height: outer_size.1,
                    };
                    let known = placement
                        .monitor
                        .as_ref()
                        .is_some_and(|name| monitor.name() == Some(name));
                    let anchors = if known {
                        geometry::infer_legacy(
                            raw,
                            work,
                            scale,
                            settings.mode == DisplayMode::ScreenEdge,
                        )
                    } else {
                        geometry::SnapAnchors::default()
                    };
                    self.inferred_anchors
                        .insert(settings.mode.key().into(), anchors);
                }
                let anchors = settings
                    .windows
                    .snap_anchors
                    .get(settings.mode.key())
                    .or_else(|| self.inferred_anchors.get(settings.mode.key()))
                    .copied()
                    .unwrap_or_default();
                let mut rect = geometry::resolve_anchors(
                    Rect {
                        x: work.x + placement.x * scale,
                        y: work.y + placement.y * scale,
                        width: outer_size.0,
                        height: outer_size.1,
                    },
                    work,
                    anchors,
                    self.gap_constraints.margins,
                    free.margins,
                    (settings.mode == DisplayMode::ScreenEdge)
                        .then_some(settings.windows.edge_side == EdgeSide::Right),
                );
                if !state.dragging {
                    frame(&shell, rect)?;
                } else if let (Ok(p), Ok(s)) = (shell.outer_position(), shell.outer_size()) {
                    rect = Rect {
                        x: p.x as f64,
                        y: p.y as f64,
                        width: s.width as f64,
                        height: s.height as f64,
                    };
                }
                native::round_corners(
                    &shell,
                    match settings.mode {
                        DisplayMode::ScreenEdge => 16.0,
                        DisplayMode::CompactHud => 12.0,
                        _ => 14.0,
                    },
                )?;
                if self.interaction.compact_visible() && self.ready_shell.is_some() {
                    native::show_without_activation(&shell)?;
                    if raise_shell {
                        native::raise_without_activation(&shell)?;
                    }
                    trigger = Some(rect);
                } else {
                    native::hide(&shell);
                    trigger = None;
                }
            }
        } else if let Some(shell) = app.get_webview_window("shell") {
            native::hide(&shell);
        }
        if trigger != self.trigger
            && self.detail_anchor == Surface::Shell
            && state.detail == DetailState::Popup
        {
            self.last_detail = DetailState::Closed;
        }
        self.trigger = trigger;
        let shell_bounds = if settings.mode == DisplayMode::SystemTray {
            None
        } else {
            trigger
        };
        if state.preview_source == Surface::Tray {
            trigger = self.tray;
        }
        let anchor_monitor = if state.preview_source == Surface::Tray {
            self.tray
                .and_then(|t| app.monitor_from_point(t.x, t.y).ok().flatten())
                .unwrap_or(monitor.clone())
        } else {
            monitor.clone()
        };
        let anchor_scale = anchor_monitor.scale_factor();
        let anchor_area = anchor_monitor.work_area();
        let anchor_work = Rect {
            x: anchor_area.position.x as f64,
            y: anchor_area.position.y as f64,
            width: anchor_area.size.width as f64,
            height: anchor_area.size.height as f64,
        };
        let mut preview_bounds = None;
        if state.shell != ShellState::Compact
            && !(if state.preview_source == Surface::Tray {
                state.tray_suppressed
            } else {
                state.suppressed
            })
            && (!state.hidden || state.preview_source == Surface::Tray)
            && !state.menu_open
        {
            if let Some(trigger) = trigger {
                if let Some(preview) = self.ensure(
                    app,
                    "preview",
                    self.preview_size,
                    service,
                    settings.windows.always_on_top || state.preview_source == Surface::Tray,
                )? {
                    native::configure(
                        &preview,
                        native::WindowPolicy {
                            tool: true,
                            no_activate: true,
                            topmost: settings.windows.always_on_top
                                || state.preview_source == Surface::Tray,
                            shadow: false,
                            standard: None,
                        },
                    )?;
                    let size =
                        native::content_outer_size(&preview, self.preview_size, anchor_scale)?;
                    let edge = (settings.mode == DisplayMode::ScreenEdge
                        && state.preview_source != Surface::Tray)
                        .then_some(settings.windows.edge_side == EdgeSide::Right);
                    let place = |trigger, width, height, gap, work| {
                        if settings.mode == DisplayMode::CompactHud
                            && state.preview_source == Surface::Shell
                        {
                            geometry::centered_panel(trigger, width, height, gap, work)
                        } else {
                            geometry::anchored(trigger, width, height, gap, work, edge)
                        }
                    };
                    let rect = place(trigger, size.0, size.1, 8.0 * anchor_scale, anchor_work);
                    frame(&preview, rect)?;
                    native::round_corners(
                        &preview,
                        if state.preview_source == Surface::Tray {
                            10.0
                        } else {
                            14.0
                        },
                    )?;
                    if self.ready_preview == Some(state.generation) {
                        native::show_without_activation(&preview)?;
                        if state.preview_source == Surface::Shell {
                            if let Some(shell) = app.get_webview_window("shell") {
                                native::above(&preview, &shell)?;
                            }
                        }
                        preview_bounds = Some(rect);
                    } else {
                        native::hide(&preview);
                    }
                }
            }
        } else if let Some(preview) = app.get_webview_window("preview") {
            native::hide(&preview);
        }
        let mut popup = None;
        let main = if state.detail != DetailState::Closed {
            self.ensure(
                app,
                "main",
                (620.0, 640.0),
                service,
                state.detail == DetailState::Popup && settings.windows.always_on_top,
            )?
        } else {
            app.get_webview_window("main")
        };
        if let Some(main) = main {
            if self.last_detail != DetailState::Closed
                && !self.fit_after_transition
                && !self.layout_dirty
            {
                if let (Ok(size), Ok(dpi)) = (main.inner_size(), main.scale_factor()) {
                    self.main_sizes[usize::from(self.last_detail == DetailState::Window)] =
                        Some((f64::from(size.width) / dpi, f64::from(size.height) / dpi));
                }
            }
            if state.detail != DetailState::Closed && !self.locked {
                let standard = state.detail == DetailState::Window;
                native::configure(
                    &main,
                    native::WindowPolicy {
                        tool: !standard,
                        no_activate: false,
                        topmost: !standard && settings.windows.always_on_top,
                        shadow,
                        standard: Some(standard),
                    },
                )?;
                if (self.last_detail != state.detail
                    || self.main_reclamp
                    || (self.layout_dirty && (!standard || !self.main_user_sized)))
                    && !(self.preview_freeze
                        && !standard
                        && self.main_page == DashboardPage::Settings)
                {
                    let index = usize::from(standard);
                    let (preferred_width, content_height) =
                        self.main_sizes[index].unwrap_or(if standard {
                            (620.0, 640.0)
                        } else {
                            (480.0, 600.0)
                        });
                    if self.last_detail != state.detail {
                        self.fit_after_transition = self.main_sizes[index].is_none();
                    }
                    let monitor = main
                        .current_monitor()
                        .ok()
                        .flatten()
                        .unwrap_or(anchor_monitor.clone());
                    let main_scale = monitor.scale_factor();
                    let area = monitor.work_area();
                    let main_work = Rect {
                        x: area.position.x as f64,
                        y: area.position.y as f64,
                        width: area.size.width as f64,
                        height: area.size.height as f64,
                    };
                    let content_height = if self.main_page == DashboardPage::Overview
                        && !(standard && self.main_user_sized)
                    {
                        self.overview_heights[index].unwrap_or(content_height)
                    } else {
                        content_height
                    };
                    let natural_size = native::content_outer_size(
                        &main,
                        (preferred_width, content_height),
                        main_scale,
                    )?;
                    let size = geometry::dashboard_size(
                        natural_size,
                        main_work,
                        self.main_user_sized && standard,
                    );
                    let rect = if standard {
                        let position = (self.last_detail == state.detail)
                            .then(|| main.outer_position().ok())
                            .flatten();
                        Rect {
                            x: position.map_or(
                                main_work.x + (main_work.width - size.0).max(0.0) / 2.0,
                                |p| p.x as f64,
                            ),
                            y: position.map_or(
                                main_work.y + (main_work.height - size.1).max(0.0) / 2.0,
                                |p| p.y as f64,
                            ),
                            width: size.0,
                            height: size.1,
                        }
                        .clamp(main_work)
                    } else {
                        let detail_trigger = if self.detail_anchor == Surface::Tray {
                            self.tray
                        } else {
                            shell_bounds
                        };
                        let detail_monitor = detail_trigger
                            .and_then(|t| app.monitor_from_point(t.x, t.y).ok().flatten())
                            .unwrap_or(anchor_monitor.clone());
                        let detail_scale = detail_monitor.scale_factor();
                        let a = detail_monitor.work_area();
                        let detail_work = Rect {
                            x: a.position.x as f64,
                            y: a.position.y as f64,
                            width: a.size.width as f64,
                            height: a.size.height as f64,
                        };
                        let edge = (settings.mode == DisplayMode::ScreenEdge
                            && self.detail_anchor != Surface::Tray)
                            .then_some(settings.windows.edge_side == EdgeSide::Right);
                        let place = |trigger, width, height, gap, work| {
                            if settings.mode == DisplayMode::Floating
                                && self.detail_anchor == Surface::Shell
                            {
                                geometry::around(trigger, width, height, gap, work)
                            } else if settings.mode == DisplayMode::CompactHud
                                && self.detail_anchor == Surface::Shell
                            {
                                geometry::centered_panel(trigger, width, height, gap, work)
                            } else {
                                geometry::anchored(trigger, width, height, gap, work, edge)
                            }
                        };
                        let popup_size = geometry::dashboard_size(
                            native::content_outer_size(
                                &main,
                                (preferred_width, content_height),
                                detail_scale,
                            )?,
                            detail_work,
                            false,
                        );
                        place(
                            detail_trigger.unwrap_or(Rect {
                                x: work.right() - 24.0 * scale,
                                y: work.bottom() - 24.0 * scale,
                                width: 24.0 * scale,
                                height: 24.0 * scale,
                            }),
                            popup_size.0,
                            popup_size.1,
                            if edge.is_some() {
                                0.0
                            } else {
                                8.0 * detail_scale
                            },
                            detail_work,
                        )
                    };
                    frame(&main, rect)?;
                    if let (Ok(actual), Ok(dpi)) = (main.inner_size(), main.scale_factor()) {
                        self.main_sizes[index] = Some((
                            f64::from(actual.width) / dpi,
                            f64::from(actual.height) / dpi,
                        ));
                    }
                    self.main_reclamp = false;
                    self.layout_dirty = false;
                }
                if standard && self.main_user_sized {
                    self.layout_dirty = false;
                }
                if !self.fit_after_transition {
                    if let (Ok(size), Ok(dpi)) = (main.inner_size(), main.scale_factor()) {
                        self.main_sizes[usize::from(standard)] =
                            Some((f64::from(size.width) / dpi, f64::from(size.height) / dpi));
                    }
                }
                if self.ready_main && !self.fit_after_transition {
                    if self.pending_focus == Some(Surface::Main) {
                        // Consume before attempting activation, including failures.
                        self.pending_focus = None;
                        activation_denied =
                            native::show_activated(&main)? == native::ActivationOutcome::Denied;
                    } else {
                        native::show_without_activation(&main)?;
                    }
                    if standard && self.attention_pending {
                        self.attention_pending = false;
                        native::request_attention(&main)?;
                    }
                    if !standard && self.detail_anchor == Surface::Shell {
                        if let Some(shell) = app.get_webview_window("shell") {
                            native::above(&main, &shell)?;
                        }
                    }
                }
                if !standard {
                    if let (Ok(p), Ok(s)) = (main.outer_position(), main.outer_size()) {
                        popup = Some(Rect {
                            x: p.x as f64,
                            y: p.y as f64,
                            width: s.width as f64,
                            height: s.height as f64,
                        });
                    }
                }
            } else {
                native::hide(&main);
            }
        }
        self.last_detail = state.detail;
        for (index, (label, needed)) in [
            ("main", state.detail != DetailState::Closed),
            ("shell", self.interaction.compact_visible()),
            (
                "preview",
                self.interaction.preview_visible() && !self.locked,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            self.idle[index].update(app.get_webview_window(label).is_some(), needed, self.now);
        }
        native::regions(native::Regions {
            generation: state.generation,
            trigger,
            shell: shell_bounds,
            tray: self.tray,
            source: state.preview_source,
            preview: preview_bounds,
            popup,
            monitor: Some(Rect {
                x: monitor.position().x as f64,
                y: monitor.position().y as f64,
                width: monitor.size().width as f64,
                height: monitor.size().height as f64,
            }),
            second_monitor: self
                .tray
                .and_then(|t| app.monitor_from_point(t.x, t.y).ok().flatten())
                .map(|m| Rect {
                    x: m.position().x as f64,
                    y: m.position().y as f64,
                    width: m.size().width as f64,
                    height: m.size().height as f64,
                }),
        });
        if activation_denied {
            self.activation_denied();
            service.dispatch_reliable(Action::Window(Input::Reconcile));
        }
        Ok(())
    }

    fn activation_denied(&mut self) {
        self.pending_focus = None;
        self.focus_due = None;
        self.attention_pending = true;
        if self.interaction.state.detail == DetailState::Popup {
            self.interaction.open_detail(true);
            self.last_detail = DetailState::Closed;
            self.main_content = None;
            self.ready_main = false;
        }
    }
}

fn frame(window: &WebviewWindow, rect: Rect) -> Result<(), &'static str> {
    native::place(window, rect)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drag_completion_rejects_cancel_lock_mode_change_and_retired_window() {
        let start = Rect {
            x: 8.0,
            y: 100.0,
            width: 60.0,
            height: 184.0,
        };
        let moved = Rect {
            x: 800.0,
            y: 250.0,
            ..start
        };
        let session = DragSession {
            window: 10,
            mode: DisplayMode::ScreenEdge,
            start,
        };
        assert!(session.accepts(10, DisplayMode::ScreenEdge, false, moved));
        assert!(!session.accepts(10, DisplayMode::ScreenEdge, false, start));
        assert!(!session.accepts(10, DisplayMode::ScreenEdge, true, moved));
        assert!(!session.accepts(10, DisplayMode::CompactHud, false, moved));
        assert!(!session.accepts(11, DisplayMode::ScreenEdge, false, moved));
        let settings = Settings {
            mode: DisplayMode::ScreenEdge,
            ..Settings::default()
        };
        for lock in [false, true] {
            let mut windows = Windows::new(&settings, false);
            windows.drag_session = Some(session);
            windows.interaction.dragging(true);
            let mut next = settings.clone();
            if lock {
                next.windows.lock_position = true;
            } else {
                next.mode = DisplayMode::CompactHud;
            }
            windows.settings_changed(&settings, &next);
            assert!(windows.drag_session.is_none());
            assert!(!windows.interaction.state.dragging);
        }
    }
    #[test]
    fn dashboard_measurements_adapt_until_actual_manual_resize() {
        let mut windows = Windows::new(&Settings::default(), true);
        let generation = windows.interaction.state.generation;
        windows.dashboard_layout(generation + 1, DashboardPage::Overview, 620.0, 510.0, true);
        assert!(windows.main_content.is_none());
        for (width, height) in [(f64::NAN, 510.0), (620.0, f64::INFINITY), (1.0, 500.0)] {
            windows.dashboard_layout(generation, DashboardPage::Overview, width, height, true);
            assert!(windows.main_content.is_none());
        }
        windows.dashboard_layout(generation, DashboardPage::Overview, 620.0, 510.0, true);
        assert!(windows.layout_dirty);
        windows.layout_dirty = false;
        windows.dashboard_layout(generation, DashboardPage::Overview, 620.0, 520.0, false);
        assert!(
            !windows.layout_dirty,
            "countdown-only measurements must not resize"
        );
        windows.dashboard_layout(generation, DashboardPage::Overview, 620.0, 530.0, true);
        assert!(
            windows.layout_dirty,
            "new Overview content refits after loading"
        );
        windows.layout_dirty = false;
        windows.main_user_sized = true;
        windows.dashboard_layout(generation, DashboardPage::Overview, 700.0, 540.0, true);
        assert!(
            !windows.layout_dirty,
            "manual resize must not trigger auto-fit"
        );
        windows.dashboard_layout(generation, DashboardPage::Overview, 700.0, 600.0, true);
        assert!(
            !windows.layout_dirty,
            "data updates must not resize the window"
        );
        windows.dashboard_layout(generation, DashboardPage::Settings, 700.0, 1200.0, true);
        assert!(
            !windows.layout_dirty,
            "navigation preserves a user-chosen standard size"
        );
        windows.layout_dirty = false;
        windows.main_user_sized = false;
        windows.main_sizes[1] = None;
        windows.fit_after_transition = true;
        windows.dashboard_layout(generation, DashboardPage::Settings, 620.0, 1000.0, true);
        assert!(windows.layout_dirty);
        assert!(!windows.fit_after_transition);
        assert_eq!(windows.main_sizes[1], Some((620.0, 640.0)));
        windows.layout_dirty = false;
        windows.dashboard_layout(generation, DashboardPage::Overview, 620.0, 300.0, true);
        assert!(windows.layout_dirty);
        assert_eq!(windows.main_sizes[1], Some((620.0, 300.0)));
        windows.layout_dirty = false;
        windows.dashboard_layout(generation, DashboardPage::Overview, 620.0, 300.0, true);
        assert!(
            !windows.layout_dirty,
            "unchanged acknowledgement must not refit"
        );
        // A work-area change while Settings is open may have clamped cached bounds.
        windows.main_sizes[1] = Some((620.0, 200.0));
        windows.dashboard_layout(generation, DashboardPage::Settings, 620.0, 1200.0, true);
        assert!(!windows.layout_dirty);
        windows.dashboard_layout(generation, DashboardPage::Overview, 620.0, 300.0, true);
        assert!(windows.layout_dirty);
        assert_eq!(windows.main_sizes[1], Some((620.0, 300.0)));
    }
    #[test]
    fn closed_dashboard_measurements_do_not_overwrite_popup_content_or_size() {
        let mut windows = Windows::new(&Settings::default(), false);
        windows.interaction.open_detail(false);
        windows.dashboard_layout(
            windows.interaction.state.generation,
            DashboardPage::Overview,
            480.0,
            581.0,
            true,
        );
        windows.interaction.close_detail();
        windows.layout_dirty = false;
        windows.dashboard_layout(
            windows.interaction.state.generation,
            DashboardPage::Overview,
            480.0,
            643.0,
            true,
        );
        assert_eq!(windows.main_sizes[0], Some((480.0, 581.0)));
        assert_eq!(windows.overview_heights[0], Some(581.0));
        assert!(!windows.layout_dirty);
    }
    #[test]
    fn dashboard_readiness_requires_the_actual_target_width_even_with_cached_sizes() {
        let mut windows = Windows::new(&Settings::default(), false);
        windows.interaction.open_detail(false);
        let generation = windows.interaction.state.generation;
        windows.main_sizes[0] = Some((480.0, 500.0));
        windows.dashboard_layout(generation, DashboardPage::Overview, 700.0, 300.0, true);
        assert!(
            !windows.ready_main,
            "old standard width must not acknowledge popup readiness"
        );
        windows.dashboard_layout(generation, DashboardPage::Overview, 480.0, 600.0, false);
        assert!(windows.ready_main);
        assert_eq!(windows.overview_heights[0], Some(600.0));
        windows.fit_after_transition = true;
        windows.main_sizes[0] = Some((320.0, 400.0));
        windows.dashboard_layout(generation, DashboardPage::Overview, 320.0, 700.0, false);
        assert!(windows.ready_main && !windows.fit_after_transition);
        assert_eq!(windows.overview_heights[0], Some(700.0));
    }
    #[test]
    fn automatic_width_clamp_refits_wrapped_overview_without_becoming_manual() {
        let mut windows = Windows::new(&Settings::default(), true);
        let generation = windows.interaction.state.generation;
        windows.dashboard_layout(generation, DashboardPage::Overview, 620.0, 510.0, true);
        // A narrow/high-DPI work area clamps client width before React acknowledges it.
        windows.main_sizes[1] = Some((500.0, 510.0));
        windows.layout_dirty = false;
        windows.dashboard_layout(generation, DashboardPage::Overview, 500.0, 650.0, false);
        assert!(windows.layout_dirty);
        assert!(!windows.main_user_sized);
        assert_eq!(windows.overview_heights[1], Some(650.0));
        windows.layout_dirty = false;
        windows.dashboard_layout(generation, DashboardPage::Overview, 500.0, 651.0, false);
        assert!(
            !windows.layout_dirty,
            "height-only countdown measurements remain ignored"
        );
    }
    #[test]
    fn popup_navigation_and_reopening_keep_a_separate_size() {
        let mut windows = Windows::new(&Settings::default(), false);
        windows.interaction.open_detail(false);
        let generation = windows.interaction.state.generation;
        windows.dashboard_layout(generation, DashboardPage::Overview, 480.0, 510.0, true);
        assert_eq!(windows.main_sizes[0], Some((480.0, 510.0)));
        assert_eq!(windows.main_sizes[1], None);
        windows.layout_dirty = false;
        for page in [DashboardPage::Settings, DashboardPage::Settings] {
            windows.dashboard_layout(generation, page, 480.0, 2400.0, true);
            assert!(!windows.layout_dirty);
            assert_eq!(windows.main_sizes[0], Some((480.0, 510.0)));
        }
        windows.interaction.close_detail();
        windows.interaction.open_detail(false);
        windows.dashboard_layout(
            windows.interaction.state.generation,
            DashboardPage::Overview,
            480.0,
            600.0,
            true,
        );
        assert_eq!(windows.main_sizes[0], Some((480.0, 600.0)));
        windows.main_user_sized = true;
        windows.dashboard_layout(
            windows.interaction.state.generation,
            DashboardPage::Overview,
            480.0,
            700.0,
            true,
        );
        assert_eq!(
            windows.main_sizes[0],
            Some((480.0, 700.0)),
            "manual standard sizing never controls popup"
        );
    }
    #[test]
    fn activation_denial_promotes_once_without_replaying_focus_or_changing_page() {
        let mut windows = Windows::new(&Settings::default(), false);
        windows.interaction.open_detail(false);
        windows.request_dashboard(DashboardPage::Settings);
        windows.pending_focus = Some(Surface::Main);
        windows.focus_due = Some(100);
        windows.activation_denied();
        assert_eq!(windows.interaction.state.detail, DetailState::Window);
        assert_eq!(windows.pending_focus, None);
        assert_eq!(windows.focus_due, None);
        assert!(windows.attention_pending);
        assert_eq!(
            windows.dashboard_request.unwrap().page,
            DashboardPage::Settings
        );
        assert!(windows.diagnostic.is_none());
        assert!(!windows.tooltip_fallback);
        let generation = windows.interaction.state.generation;
        windows.activation_denied();
        assert_eq!(generation, windows.interaction.state.generation);
    }
    #[test]
    fn reselect_reveals_hidden_shell_and_raises_once_after_current_hydration() {
        let settings = Settings {
            mode: DisplayMode::Floating,
            ..Settings::default()
        };
        let mut windows = Windows::new(&settings, false);
        windows.toggle_hidden(settings.mode);
        assert!(windows.interaction.state.hidden);
        windows.ready_shell = Some(windows.interaction.state.generation);
        windows.reveal_mode(settings.mode);
        assert!(!windows.interaction.state.hidden);
        assert!(!windows.take_raise(settings.mode));
        assert_eq!(windows.pending_raise, Some(settings.mode));
        windows.ready_shell = Some(windows.interaction.state.generation);
        assert!(windows.take_raise(settings.mode));
        assert!(!windows.take_raise(settings.mode));
        assert!(!settings.windows.always_on_top);
    }

    #[test]
    fn external_foreground_restores_topmost_once_without_revealing_hidden_widgets() {
        for mode in [
            DisplayMode::Floating,
            DisplayMode::ScreenEdge,
            DisplayMode::CompactHud,
        ] {
            let mut settings = Settings {
                mode,
                ..Settings::default()
            };
            settings.windows.always_on_top = true;
            let mut windows = Windows::new(&settings, false);
            windows.ready_shell = Some(windows.interaction.state.generation);
            windows.external_foreground(&settings);
            windows.external_foreground(&settings);
            assert!(windows.take_raise(mode));
            assert!(!windows.take_raise(mode));
            windows.toggle_hidden(mode);
            windows.external_foreground(&settings);
            assert!(windows.interaction.state.hidden);
            assert!(!windows.take_raise(mode));
        }
    }

    #[test]
    fn external_foreground_respects_topmost_off_suppression_and_dragging() {
        for (enabled, suppressed, dragging) in [
            (false, false, false),
            (true, true, false),
            (true, false, true),
        ] {
            let mut settings = Settings {
                mode: DisplayMode::Floating,
                ..Settings::default()
            };
            settings.windows.always_on_top = enabled;
            let mut windows = Windows::new(&settings, false);
            windows.interaction.suppress(suppressed);
            windows.interaction.dragging(dragging);
            windows.external_foreground(&settings);
            assert_eq!(windows.pending_raise, None);
        }
        let mut settings = Settings::default();
        settings.windows.always_on_top = true;
        let mut windows = Windows::new(&settings, false);
        windows.external_foreground(&settings);
        assert_eq!(windows.pending_raise, None);
    }

    #[test]
    fn disabling_topmost_cancels_pending_external_repair() {
        let mut settings = Settings {
            mode: DisplayMode::Floating,
            ..Settings::default()
        };
        settings.windows.always_on_top = true;
        let mut windows = Windows::new(&settings, false);
        windows.external_foreground(&settings);
        let mut next = settings.clone();
        next.windows.always_on_top = false;
        windows.settings_changed(&settings, &next);
        windows.ready_shell = Some(windows.interaction.state.generation);
        assert!(!windows.take_raise(next.mode));
    }

    #[test]
    fn mode_changes_reveal_but_unrelated_settings_preserve_hidden_state() {
        let mut settings = Settings {
            mode: DisplayMode::Floating,
            ..Settings::default()
        };
        let mut windows = Windows::new(&settings, false);
        windows.toggle_hidden(settings.mode);
        let mut next = settings.clone();
        next.theme_preference = crate::model::ThemePreference::Dark;
        windows.settings_changed(&settings, &next);
        assert!(windows.interaction.state.hidden);
        assert_eq!(windows.pending_raise, None);
        for mode in [
            DisplayMode::ScreenEdge,
            DisplayMode::CompactHud,
            DisplayMode::Floating,
        ] {
            next.mode = mode;
            windows.settings_changed(&settings, &next);
            assert!(!windows.interaction.state.hidden);
            assert_eq!(windows.pending_raise, Some(mode));
            settings = next.clone();
            windows.toggle_hidden(mode);
        }
        windows.toggle_hidden(settings.mode);
        assert!(!windows.interaction.state.hidden);
        assert_eq!(windows.pending_raise, Some(settings.mode));
    }

    #[test]
    fn pending_reveal_is_canceled_by_hide_tray_switch_and_suppression() {
        let settings = Settings {
            mode: DisplayMode::Floating,
            ..Settings::default()
        };
        let mut windows = Windows::new(&settings, false);
        windows.reveal_mode(settings.mode);
        windows.toggle_hidden(settings.mode);
        assert_eq!(windows.pending_raise, None);
        windows.reveal_mode(settings.mode);
        windows.reveal_mode(DisplayMode::SystemTray);
        assert_eq!(windows.pending_raise, None);
        windows.reveal_mode(settings.mode);
        windows.interaction.suppress(true);
        assert!(!windows.take_raise(settings.mode));
        windows.interaction.suppress(false);
        windows.ready_shell = Some(windows.interaction.state.generation);
        assert!(!windows.take_raise(settings.mode));
        windows.reveal_mode(settings.mode);
        assert!(!windows.take_raise(DisplayMode::ScreenEdge));
        assert_eq!(windows.pending_raise, None);
    }

    #[test]
    fn preview_measurements_are_valid_only_for_the_current_generation() {
        let mut windows = Windows::new(&Settings::default(), false);
        windows.ready_preview = Some(windows.interaction.state.generation);
        assert_eq!(windows.preview_content_size(), Some((260.0, 76.0)));
        windows.interaction.collapse();
        assert_eq!(windows.preview_content_size(), None);
    }
    #[test]
    fn widget_settings_cancels_pending_overview_and_keeps_popup_or_window() {
        let settings = Settings {
            mode: DisplayMode::Floating,
            ..Settings::default()
        };
        for standard in [false, true] {
            let mut windows = Windows::new(&settings, standard);
            windows.clicks.single(0, 500, Surface::Shell);
            assert_eq!(
                windows.open_widget_settings(Surface::Shell),
                Some(Surface::Main)
            );
            assert_eq!(windows.clicks.deadline(), None);
            assert_eq!(windows.detail_anchor, Surface::Shell);
            assert_eq!(
                windows.interaction.state.detail,
                if standard {
                    DetailState::Window
                } else {
                    DetailState::Popup
                }
            );
            assert!(matches!(
                windows.dashboard_request.unwrap().page,
                DashboardPage::Settings
            ));
        }
    }
    #[test]
    fn widget_settings_rejects_unrelated_sources_and_extra_payload_fields() {
        let mut windows = Windows::new(&Settings::default(), false);
        assert_eq!(windows.open_widget_settings(Surface::Main), None);
        assert_eq!(windows.open_widget_settings(Surface::Tray), None);
        assert_eq!(windows.interaction.state.detail, DetailState::Closed);
        assert!(windows.dashboard_request.is_none());
        assert!(matches!(
            serde_json::from_str::<Request>(r#"{"type":"settings"}"#).unwrap(),
            Request::Settings {}
        ));
        assert!(serde_json::from_str::<Request>(r#"{"type":"settings","page":"other"}"#).is_err());
    }
}
