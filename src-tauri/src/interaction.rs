//! Native presentation policy, independent of quota status and any particular webview.
use crate::model::DisplayMode;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ShellState {
    #[default]
    Compact,
    Preview,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DetailState {
    #[default]
    Closed,
    Popup,
    Window,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Surface {
    #[default]
    Tray,
    Shell,
    Preview,
    Main,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presentation {
    pub generation: u64,
    pub shell: ShellState,
    pub detail: DetailState,
    pub hidden: bool,
    pub suppressed: bool,
    pub menu_open: bool,
    pub dragging: bool,
    pub preview_source: Surface,
    pub tray_suppressed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    FocusDetail,
}

/// A click must survive the OS double-click interval before it can open a popup.
#[derive(Default)]
pub struct ClickActivation {
    pending: Option<(u64, Surface)>,
    ignore_until: u64,
}
impl ClickActivation {
    pub fn single(&mut self, now: u64, interval: u64, source: Surface) {
        if now >= self.ignore_until && self.pending.is_none() {
            self.pending = Some((now.saturating_add(interval), source));
        }
    }
    pub fn double(&mut self, now: u64, interval: u64) {
        self.cancel();
        // Some tray adapters emit a final button-up after DoubleClick.
        self.ignore_until = now.saturating_add(interval);
    }
    pub fn cancel(&mut self) {
        self.pending = None;
    }
    pub fn deadline(&self) -> Option<u64> {
        self.pending.map(|(at, _)| at)
    }
    pub fn take_due(&mut self, now: u64) -> Option<Surface> {
        if self.pending.is_some_and(|(at, _)| now >= at) {
            self.pending.take().map(|(_, source)| source)
        } else {
            None
        }
    }
}
#[derive(Clone, Copy, Debug)]
enum TimerKind {
    Dwell,
    Leave,
}

pub struct Interaction {
    pub state: Presentation,
    pub mode: DisplayMode,
    trigger: bool,
    preview: bool,
    corridor: bool,
    blocked_until_exit: bool,
    timer: Option<(u64, u64, TimerKind)>,
}

impl Interaction {
    pub fn new(mode: DisplayMode, detail: DetailState) -> Self {
        Self {
            state: Presentation {
                detail,
                preview_source: if mode == DisplayMode::SystemTray {
                    Surface::Tray
                } else {
                    Surface::Shell
                },
                ..Presentation::default()
            },
            mode,
            trigger: false,
            preview: false,
            corridor: false,
            blocked_until_exit: detail != DetailState::Closed,
            timer: None,
        }
    }
    fn invalidate(&mut self) {
        self.state.generation += 1;
        self.timer = None;
    }
    pub fn retire_view(&mut self) {
        self.invalidate();
    }
    fn can_preview(&self) -> bool {
        !(self.mode == DisplayMode::Floating && self.state.preview_source == Surface::Shell)
            && self.state.detail == DetailState::Closed
            && !self.state.menu_open
            && !self.state.dragging
            && !(if self.state.preview_source == Surface::Tray {
                self.state.tray_suppressed
            } else {
                self.state.suppressed
            })
            && (!self.state.hidden || self.state.preview_source == Surface::Tray)
            && !self.blocked_until_exit
            && self.state.shell == ShellState::Compact
    }
    #[cfg(test)]
    pub fn pointer(
        &mut self,
        trigger: bool,
        preview: bool,
        corridor: bool,
        now: u64,
        hover_enabled: bool,
    ) {
        self.pointer_from(
            self.state.preview_source,
            trigger,
            preview,
            corridor,
            now,
            hover_enabled,
        );
    }
    pub fn pointer_from(
        &mut self,
        source: Surface,
        trigger: bool,
        preview: bool,
        corridor: bool,
        now: u64,
        hover_enabled: bool,
    ) {
        if trigger && source != self.state.preview_source {
            self.invalidate();
            self.state.preview_source = source;
            self.state.shell = ShellState::Compact;
            self.trigger = false;
        }
        let entered = trigger && !self.trigger;
        self.trigger = trigger;
        self.preview = preview;
        self.corridor = corridor;
        if !trigger && !preview && !corridor {
            self.blocked_until_exit = false;
            if self.state.shell == ShellState::Preview {
                self.timer = Some((now + 350, self.state.generation, TimerKind::Leave));
            } else {
                self.timer = None;
            }
        } else if preview || corridor {
            self.timer = None;
        } else if entered && hover_enabled && self.can_preview() {
            self.timer = Some((now + 200, self.state.generation, TimerKind::Dwell));
        } else if trigger && matches!(self.timer, Some((_, _, TimerKind::Leave))) {
            self.timer = None;
        }
    }
    pub fn next_deadline(&self) -> Option<u64> {
        self.timer.map(|(at, _, _)| at)
    }
    pub fn tick(&mut self, now: u64) -> bool {
        let Some((at, generation, kind)) = self.timer else {
            return false;
        };
        if now < at {
            return false;
        }
        self.timer = None;
        if generation != self.state.generation {
            return false;
        }
        match kind {
            TimerKind::Dwell if self.trigger && self.can_preview() => {
                self.state.shell = ShellState::Preview;
                self.state.generation += 1;
                true
            }
            TimerKind::Leave
                if !self.trigger
                    && !self.preview
                    && !self.corridor
                    && self.state.shell == ShellState::Preview =>
            {
                self.state.shell = ShellState::Compact;
                self.state.generation += 1;
                true
            }
            _ => false,
        }
    }
    pub fn activate(&mut self, source: Surface) -> Option<Effect> {
        self.activate_from(source)
    }
    pub fn activate_from(&mut self, source: Surface) -> Option<Effect> {
        if self.state.dragging {
            return None;
        }
        self.state.preview_source = source;
        self.invalidate();
        self.blocked_until_exit = true;
        self.state.shell = ShellState::Compact;
        self.state.detail = if self.state.detail == DetailState::Window {
            DetailState::Window
        } else {
            DetailState::Popup
        };
        Some(Effect::FocusDetail)
    }
    pub fn open_detail(&mut self, standard: bool) {
        self.invalidate();
        self.blocked_until_exit = true;
        if self.state.shell == ShellState::Preview {
            self.state.shell = ShellState::Compact;
        }
        self.state.detail = if standard || self.state.detail == DetailState::Window {
            DetailState::Window
        } else {
            DetailState::Popup
        };
    }
    pub fn close_detail(&mut self) {
        self.invalidate();
        self.state.detail = DetailState::Closed;
        self.blocked_until_exit = self.trigger || self.preview || self.corridor;
    }
    pub fn outside(&mut self) -> bool {
        if self.state.detail != DetailState::Popup || self.state.menu_open {
            return false;
        }
        self.close_detail();
        true
    }
    pub fn escape(&mut self, _source: Surface) -> bool {
        if self.state.menu_open {
            return false;
        }
        if self.state.detail == DetailState::Popup {
            self.close_detail();
            return true;
        }
        false
    }
    pub fn collapse(&mut self) {
        self.invalidate();
        self.state.shell = ShellState::Compact;
        self.blocked_until_exit = true;
    }
    pub fn menu(&mut self, open: bool) {
        self.invalidate();
        self.state.menu_open = open;
        self.blocked_until_exit = true;
        if self.state.shell == ShellState::Preview {
            self.state.shell = ShellState::Compact;
        }
        if open && self.state.detail == DetailState::Popup {
            self.state.detail = DetailState::Closed;
        }
    }
    pub fn change_mode(&mut self, mode: DisplayMode) {
        self.invalidate();
        self.mode = mode;
        self.state.shell = ShellState::Compact;
        if self.state.detail == DetailState::Popup {
            self.state.detail = DetailState::Closed;
        }
        self.trigger = false;
        self.preview = false;
        self.corridor = false;
        self.blocked_until_exit = true;
    }
    pub fn dragging(&mut self, value: bool) {
        self.invalidate();
        self.state.dragging = value;
        self.blocked_until_exit = true;
        if self.state.shell == ShellState::Preview {
            self.state.shell = ShellState::Compact;
        }
    }
    pub fn suppress(&mut self, value: bool) {
        if self.state.suppressed != value {
            self.invalidate();
            self.state.suppressed = value;
            self.blocked_until_exit = true;
            if self.state.shell == ShellState::Preview && self.state.preview_source != Surface::Tray
            {
                self.state.shell = ShellState::Compact;
            }
        }
    }
    pub fn hidden(&mut self, value: bool) {
        self.invalidate();
        self.state.hidden = value;
        self.state.shell = ShellState::Compact;
        self.blocked_until_exit = true;
    }
    pub fn suppress_tray(&mut self, value: bool) {
        if self.state.tray_suppressed != value {
            self.invalidate();
            self.state.tray_suppressed = value;
            self.blocked_until_exit = true;
            if self.state.preview_source == Surface::Tray && self.state.shell == ShellState::Preview
            {
                self.state.shell = ShellState::Compact;
            }
        }
    }
    pub fn expanded_visible(&self) -> bool {
        self.state.detail != DetailState::Closed || self.preview_visible()
    }
    pub fn preview_visible(&self) -> bool {
        !self.state.menu_open
            && (self.state.shell != ShellState::Compact
                && if self.state.preview_source == Surface::Tray {
                    !self.state.tray_suppressed
                } else {
                    !self.state.suppressed && !self.state.hidden
                })
    }
    pub fn compact_visible(&self) -> bool {
        self.mode != DisplayMode::SystemTray && !self.state.hidden && !self.state.suppressed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_click_waits_for_os_interval_and_double_click_cancels_popup() {
        let mut clicks = ClickActivation::default();
        clicks.single(100, 500, Surface::Shell);
        assert_eq!(clicks.deadline(), Some(600));
        assert_eq!(clicks.take_due(599), None);
        clicks.double(450, 500);
        assert_eq!(clicks.take_due(600), None);
        clicks.single(460, 500, Surface::Tray);
        assert_eq!(clicks.deadline(), None);
        clicks.single(1000, 500, Surface::Tray);
        assert_eq!(clicks.take_due(1500), Some(Surface::Tray));
        assert_eq!(clicks.take_due(1600), None);
    }
    #[test]
    fn repeated_button_up_keeps_first_deadline_and_cancellation_discards_it() {
        let mut clicks = ClickActivation::default();
        clicks.single(0, 900, Surface::Shell);
        clicks.single(10, 900, Surface::Shell);
        assert_eq!(clicks.deadline(), Some(900));
        clicks.cancel();
        assert_eq!(clicks.take_due(1000), None);
    }
    #[test]
    fn tray_preview_remains_available_when_desktop_shell_is_hidden_or_suppressed() {
        let mut i = Interaction::new(DisplayMode::Floating, DetailState::Closed);
        i.hidden(true);
        i.suppress(true);
        i.pointer_from(Surface::Tray, false, false, false, 0, true);
        i.pointer_from(Surface::Tray, true, false, false, 1, true);
        assert!(i.tick(201));
        assert_eq!(i.state.preview_source, Surface::Tray);
        assert!(!i.compact_visible());
        i.suppress_tray(true);
        assert_eq!(i.state.shell, ShellState::Compact);
    }
    #[test]
    fn edge_widget_and_tray_activation_open_detail_and_repeated_clicks_focus_it() {
        let mut i = Interaction::new(DisplayMode::ScreenEdge, DetailState::Closed);
        assert_eq!(i.activate(Surface::Shell), Some(Effect::FocusDetail));
        assert_eq!(i.state.detail, DetailState::Popup);
        assert_eq!(i.state.shell, ShellState::Compact);
        assert_eq!(i.activate_from(Surface::Tray), Some(Effect::FocusDetail));
        assert_eq!(i.state.detail, DetailState::Popup);
        assert_eq!(i.state.preview_source, Surface::Tray);
        assert!(i.outside());
        assert_eq!(i.state.shell, ShellState::Compact);
        assert_eq!(i.state.detail, DetailState::Closed);
    }

    #[test]
    fn desktop_modes_share_hover_dwell_and_leave_grace() {
        for mode in [DisplayMode::ScreenEdge, DisplayMode::CompactHud] {
            let mut i = Interaction::new(mode, DetailState::Closed);
            i.pointer_from(Surface::Shell, true, false, false, 0, true);
            assert!(!i.tick(199), "{mode:?} opened before the dwell");
            assert!(i.tick(200), "{mode:?} missed the shared dwell");
            assert_eq!(i.state.shell, ShellState::Preview);
            i.pointer_from(Surface::Shell, false, false, false, 210, true);
            assert!(!i.tick(559), "{mode:?} dismissed before the shared grace");
            assert!(i.tick(560), "{mode:?} missed the shared leave grace");
            assert_eq!(i.state.shell, ShellState::Compact);
        }
    }

    #[test]
    fn floating_has_no_shell_hover_but_keeps_tray_hover_and_clicks() {
        let mut i = Interaction::new(DisplayMode::Floating, DetailState::Closed);
        i.pointer_from(Surface::Shell, true, false, false, 0, true);
        assert_eq!(i.next_deadline(), None);
        assert!(!i.tick(1000));
        assert_eq!(i.state.shell, ShellState::Compact);
        i.pointer_from(Surface::Tray, true, false, false, 1100, true);
        assert!(i.tick(1300));
        assert_eq!(i.state.preview_source, Surface::Tray);
        assert_eq!(i.activate(Surface::Shell), Some(Effect::FocusDetail));
        assert_eq!(i.state.detail, DetailState::Popup);
        i.close_detail();
        i.change_mode(DisplayMode::CompactHud);
        i.pointer_from(Surface::Shell, true, false, false, 2000, true);
        i.change_mode(DisplayMode::Floating);
        assert!(!i.tick(2200));
        assert_eq!(i.state.shell, ShellState::Compact);
    }

    #[test]
    fn every_desktop_mode_click_opens_or_raises_dashboard() {
        for mode in [
            DisplayMode::Floating,
            DisplayMode::ScreenEdge,
            DisplayMode::CompactHud,
        ] {
            let mut i = Interaction::new(mode, DetailState::Closed);
            assert_eq!(i.activate(Surface::Shell), Some(Effect::FocusDetail));
            assert_eq!(i.state.detail, DetailState::Popup);
            assert_eq!(i.state.shell, ShellState::Compact);
            assert_eq!(i.activate(Surface::Shell), Some(Effect::FocusDetail));
            assert_eq!(i.state.detail, DetailState::Popup);
        }
    }
    #[test]
    fn double_click_promotion_keeps_the_dashboard_open_as_a_standard_window() {
        let mut i = Interaction::new(DisplayMode::Floating, DetailState::Closed);
        assert_eq!(i.activate(Surface::Shell), Some(Effect::FocusDetail));
        assert_eq!(i.state.detail, DetailState::Popup);
        i.open_detail(true);
        assert_eq!(i.state.detail, DetailState::Window);
        assert!(!i.outside());
        assert!(!i.escape(Surface::Main));
    }
    #[test]
    fn changing_hover_source_invalidates_the_old_preview_timer() {
        let mut i = Interaction::new(DisplayMode::Floating, DetailState::Closed);
        i.pointer_from(Surface::Shell, true, false, false, 0, true);
        let generation = i.state.generation;
        i.pointer_from(Surface::Tray, true, false, false, 100, true);
        assert!(i.state.generation > generation);
        assert!(!i.tick(200));
        assert!(i.tick(300));
        assert_eq!(i.state.preview_source, Surface::Tray);
    }
    #[test]
    fn dwell_corridor_and_leave_grace() {
        let mut i = Interaction::new(DisplayMode::SystemTray, DetailState::Closed);
        i.pointer(true, false, false, 0, true);
        assert!(!i.tick(199));
        assert!(i.tick(200));
        i.pointer(false, false, true, 210, true);
        assert!(!i.tick(1000));
        i.pointer(false, true, false, 1001, true);
        assert!(!i.tick(2000));
        i.pointer(false, false, false, 2001, true);
        assert!(!i.tick(2350));
        assert!(i.tick(2351));
        assert_eq!(i.state.shell, ShellState::Compact);
    }
    #[test]
    fn entry_cancels_dismissal_and_detail_requires_fresh_entry() {
        let mut i = Interaction::new(DisplayMode::CompactHud, DetailState::Closed);
        i.pointer(true, false, false, 0, true);
        i.tick(200);
        i.pointer(false, false, false, 210, true);
        i.pointer(false, true, false, 500, true);
        assert!(!i.tick(600));
        i.open_detail(false);
        i.close_detail();
        i.pointer(true, false, false, 700, true);
        assert!(!i.tick(1000));
        i.pointer(false, false, false, 1001, true);
        i.pointer(true, false, false, 1002, true);
        assert!(i.tick(1202));
    }
    #[test]
    fn edge_hover_preview_dismisses_and_details_close_normally() {
        let mut i = Interaction::new(DisplayMode::ScreenEdge, DetailState::Closed);
        i.pointer_from(Surface::Shell, true, false, false, 0, true);
        assert!(i.tick(200));
        i.pointer_from(Surface::Shell, false, false, false, 210, true);
        assert!(i.tick(560));
        assert_eq!(i.state.shell, ShellState::Compact);
        i.open_detail(false);
        assert!(i.outside());
        assert_eq!(i.state.shell, ShellState::Compact);
        assert!(!i.escape(Surface::Main));
    }
    #[test]
    fn generation_menu_drag_and_activation_cancel_stale_work() {
        let mut i = Interaction::new(DisplayMode::CompactHud, DetailState::Closed);
        i.pointer(true, false, false, 0, true);
        i.menu(true);
        assert!(!i.tick(200));
        i.menu(false);
        i.pointer(false, false, false, 201, true);
        i.pointer(true, false, false, 202, true);
        i.dragging(true);
        assert!(!i.tick(500));
        assert_eq!(i.activate(Surface::Shell), None);
        i.dragging(false);
        assert_eq!(i.activate(Surface::Shell), Some(Effect::FocusDetail));
        assert_eq!(i.activate(Surface::Shell), Some(Effect::FocusDetail));
        assert_eq!(i.state.detail, DetailState::Popup);
        i.change_mode(DisplayMode::Floating);
        assert!(!i.tick(1000));
        assert_eq!(i.state.detail, DetailState::Closed);
        i.open_detail(true);
        i.change_mode(DisplayMode::ScreenEdge);
        assert_eq!(i.state.detail, DetailState::Window);
        assert!(!i.outside());
        assert!(!i.escape(Surface::Main));
    }
    #[test]
    fn fullscreen_suppresses_automatic_shells_but_not_explicit_detail() {
        let mut i = Interaction::new(DisplayMode::Floating, DetailState::Closed);
        i.suppress(true);
        assert!(!i.compact_visible());
        i.pointer(true, false, false, 0, true);
        assert!(!i.tick(200));
        i.open_detail(true);
        assert!(i.expanded_visible());
        assert_eq!(i.state.detail, DetailState::Window);
    }
    #[test]
    fn suppressed_edge_hover_does_not_count_as_visible_expanded_detail() {
        let mut i = Interaction::new(DisplayMode::ScreenEdge, DetailState::Closed);
        i.pointer_from(Surface::Shell, true, false, false, 0, true);
        i.tick(200);
        assert!(i.expanded_visible());
        i.suppress(true);
        assert!(!i.expanded_visible());
        assert_eq!(i.state.shell, ShellState::Compact);
        i.suppress(false);
        i.pointer_from(Surface::Shell, false, false, false, 201, true);
        i.pointer_from(Surface::Shell, true, false, false, 202, true);
        assert!(i.tick(402));
        assert!(i.expanded_visible());
        i.open_detail(true);
        i.suppress(true);
        assert!(i.expanded_visible());
    }
}
