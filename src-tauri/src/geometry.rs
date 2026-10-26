use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HorizontalAnchor {
    #[default]
    None,
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAnchor {
    #[default]
    None,
    Top,
    Bottom,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct SnapAnchors {
    pub horizontal: HorizontalAnchor,
    pub vertical: VerticalAnchor,
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GapConstraints {
    pub margins: [f64; 4],
    pub taskbar_uncertain: bool,
    pub reduced_to_fit: bool,
    pub content_exceeds_work_area: bool,
}

/// Margins are physical pixels in left/top/right/bottom order. Fit margins
/// rather than silently resizing the content to satisfy a requested gap.
pub fn gap_constraints(
    work: Rect,
    size: (f64, f64),
    scale: f64,
    gap: u8,
    safety: [bool; 4],
    uncertain: bool,
) -> GapConstraints {
    let mut margins =
        safety.map(|safe| (f64::from(gap).max(if safe { 8.0 } else { 0.0 }) * scale).round());
    let mut reduced = false;
    for (a, b, available) in [
        (0, 2, (work.width - size.0).max(0.0)),
        (1, 3, (work.height - size.1).max(0.0)),
    ] {
        let total = margins[a] + margins[b];
        if total > available {
            let factor = available / total;
            margins[a] = (margins[a] * factor).floor();
            margins[b] = (margins[b] * factor).floor();
            reduced = true;
        }
    }
    GapConstraints {
        margins,
        taskbar_uncertain: uncertain,
        reduced_to_fit: reduced,
        content_exceeds_work_area: size.0 > work.width || size.1 > work.height,
    }
}
pub fn resolve_anchors(
    mut rect: Rect,
    work: Rect,
    anchors: SnapAnchors,
    margins: [f64; 4],
    free_safety: [f64; 4],
    edge_right: Option<bool>,
) -> Rect {
    let horizontal = edge_right
        .map(|right| {
            if right {
                HorizontalAnchor::Right
            } else {
                HorizontalAnchor::Left
            }
        })
        .unwrap_or(anchors.horizontal);
    rect.x = match horizontal {
        HorizontalAnchor::Left => work.x + margins[0],
        HorizontalAnchor::Right => work.right() - margins[2] - rect.width,
        HorizontalAnchor::None => rect.x,
    };
    rect.y = match anchors.vertical {
        VerticalAnchor::Top => work.y + margins[1],
        VerticalAnchor::Bottom => work.bottom() - margins[3] - rect.height,
        VerticalAnchor::None => rect.y,
    };
    let vertical = if edge_right.is_some() {
        margins
    } else {
        free_safety
    };
    rect.x = rect.x.clamp(
        work.x + free_safety[0],
        (work.right() - rect.width - free_safety[2]).max(work.x + free_safety[0]),
    );
    rect.y = rect.y.clamp(
        work.y + vertical[1],
        (work.bottom() - rect.height - vertical[3]).max(work.y + vertical[1]),
    );
    rect
}
fn choose_axis(
    position: f64,
    size: f64,
    work: (f64, f64),
    before: f64,
    after: f64,
    attraction: f64,
    previous: i8,
) -> i8 {
    let low = work.0 + before;
    let high = work.1 - after - size;
    let left = position <= low + attraction;
    let right = position >= high - attraction;
    match (left, right) {
        (true, true) => {
            let a = (position - low).abs();
            let b = (position - high).abs();
            if a == b {
                if previous == 0 {
                    -1
                } else {
                    previous
                }
            } else if a < b {
                -1
            } else {
                1
            }
        }
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    }
}
pub fn release_anchors(
    rect: Rect,
    work: Rect,
    margins: [f64; 4],
    scale: f64,
    previous: SnapAnchors,
    edge: bool,
) -> SnapAnchors {
    let h = choose_axis(
        rect.x,
        rect.width,
        (work.x, work.right()),
        margins[0],
        margins[2],
        12.0 * scale,
        match previous.horizontal {
            HorizontalAnchor::Left => -1,
            HorizontalAnchor::Right => 1,
            _ => 0,
        },
    );
    let v = choose_axis(
        rect.y,
        rect.height,
        (work.y, work.bottom()),
        margins[1],
        margins[3],
        12.0 * scale,
        match previous.vertical {
            VerticalAnchor::Top => -1,
            VerticalAnchor::Bottom => 1,
            _ => 0,
        },
    );
    SnapAnchors {
        horizontal: if edge {
            HorizontalAnchor::None
        } else {
            match h {
                -1 => HorizontalAnchor::Left,
                1 => HorizontalAnchor::Right,
                _ => HorizontalAnchor::None,
            }
        },
        vertical: match v {
            -1 => VerticalAnchor::Top,
            1 => VerticalAnchor::Bottom,
            _ => VerticalAnchor::None,
        },
    }
}
pub fn infer_legacy(rect: Rect, work: Rect, scale: f64, edge: bool) -> SnapAnchors {
    let inset = if edge { 8.0 * scale } else { 0.0 };
    SnapAnchors {
        horizontal: if edge {
            HorizontalAnchor::None
        } else if (rect.x - work.x).abs() <= scale {
            HorizontalAnchor::Left
        } else if (rect.right() - work.right()).abs() <= scale {
            HorizontalAnchor::Right
        } else {
            HorizontalAnchor::None
        },
        vertical: if (rect.y - work.y - inset).abs() <= scale {
            VerticalAnchor::Top
        } else if (rect.bottom() - work.bottom() + inset).abs() <= scale {
            VerticalAnchor::Bottom
        } else {
            VerticalAnchor::None
        },
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Rect {
    pub fn right(self) -> f64 {
        self.x + self.width
    }
    pub fn bottom(self) -> f64 {
        self.y + self.height
    }
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }
    pub fn covers_monitor(self, monitor: Rect) -> bool {
        self.x <= monitor.x + 2.0
            && self.y <= monitor.y + 2.0
            && self.right() >= monitor.right() - 2.0
            && self.bottom() >= monitor.bottom() - 2.0
    }
    pub fn clamp(self, work: Rect) -> Self {
        let width = self.width.min(work.width).max(1.0);
        let height = self.height.min(work.height).max(1.0);
        Self {
            x: self.x.clamp(work.x, work.right() - width),
            y: self.y.clamp(work.y, work.bottom() - height),
            width,
            height,
        }
    }
}

// Content is logical client pixels; shadow insets are measured physical pixels.
pub fn content_outer_size(content: (f64, f64), scale: f64, frame: (u32, u32)) -> (f64, f64) {
    (
        (content.0 * scale).ceil() + f64::from(frame.0),
        (content.1 * scale).ceil() + f64::from(frame.1),
    )
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Placement {
    pub monitor: Option<String>,
    pub x: f64,
    pub y: f64,
}
impl Placement {
    pub fn validate(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.x.abs() <= 100_000.0
            && self.y.abs() <= 100_000.0
            && self.monitor.as_ref().is_none_or(|s| s.len() <= 256)
    }
    // Placement offsets are logical; the outer size is already physical.
    #[cfg(test)]
    pub fn physical(&self, work: Rect, scale: f64, size: (f64, f64)) -> Rect {
        Rect {
            x: work.x + self.x * scale,
            y: work.y + self.y * scale,
            width: size.0,
            height: size.1,
        }
        .clamp(work)
    }
    pub fn from_physical(rect: Rect, work: Rect, scale: f64, monitor: Option<String>) -> Self {
        Self {
            monitor,
            x: (rect.x - work.x) / scale,
            y: (rect.y - work.y) / scale,
        }
    }
}

/// Dock a released Edge shell, retaining its side on an exact center tie.
#[cfg(test)]
pub fn dock_edge(mut rect: Rect, work: Rect, scale: f64, previous_right: bool) -> (Rect, bool) {
    let delta = rect.x + rect.width / 2.0 - (work.x + work.width / 2.0);
    let right = if delta == 0.0 {
        previous_right
    } else {
        delta > 0.0
    };
    let inset = 8.0 * scale;
    rect.x = if right {
        work.right() - inset - rect.width
    } else {
        work.x + inset
    };
    (
        rect.clamp(Rect {
            x: work.x + inset,
            y: work.y + inset,
            width: (work.width - 2.0 * inset).max(1.0),
            height: (work.height - 2.0 * inset).max(1.0),
        }),
        right,
    )
}

/// Squared distance to a monitor rectangle, including negative desktop coordinates.
pub fn point_distance(rect: Rect, point: (f64, f64)) -> f64 {
    let dx = (rect.x - point.0).max(0.0).max(point.0 - rect.right());
    let dy = (rect.y - point.1).max(0.0).max(point.1 - rect.bottom());
    dx * dx + dy * dy
}

pub fn anchored(
    trigger: Rect,
    width: f64,
    height: f64,
    gap: f64,
    work: Rect,
    edge: Option<bool>,
) -> Rect {
    let (x, y) = if let Some(right) = edge {
        (
            if right {
                trigger.x - gap - width
            } else {
                trigger.right() + gap
            },
            trigger.y,
        )
    } else {
        let above = trigger.y - gap - height;
        let below = trigger.bottom() + gap;
        (
            trigger.right() - width,
            if above >= work.y {
                above
            } else if below + height <= work.bottom() {
                below
            } else {
                above
            },
        )
    };
    Rect {
        x,
        y,
        width,
        height,
    }
    .clamp(work)
}

/// HUD panels are centered on the actual trigger, then constrained to usable space.
pub fn centered_panel(trigger: Rect, width: f64, height: f64, gap: f64, work: Rect) -> Rect {
    let mut rect = anchored(trigger, width, height, gap, work, None);
    rect.x = trigger.x + (trigger.width - rect.width) / 2.0;
    rect.clamp(work)
}

fn overlap(a: Rect, b: Rect) -> f64 {
    (a.right().min(b.right()) - a.x.max(b.x)).max(0.0)
        * (a.bottom().min(b.bottom()) - a.y.max(b.y)).max(0.0)
}

// Preserve above/below preference, but use a side when tall Floating cards
// leave too little vertical space. All candidates include work-area clamping.
pub fn around(trigger: Rect, width: f64, height: f64, gap: f64, work: Rect) -> Rect {
    let width = width.min(work.width).max(1.0);
    let height = height.min(work.height).max(1.0);
    let mut best = None;
    for (x, y) in [
        (trigger.right() - width, trigger.y - gap - height),
        (trigger.right() - width, trigger.bottom() + gap),
        (trigger.right() + gap, trigger.y),
        (trigger.x - gap - width, trigger.y),
    ] {
        let rect = Rect {
            x,
            y,
            width,
            height,
        }
        .clamp(work);
        let area = overlap(rect, trigger);
        if best.is_none_or(|(_, previous)| area < previous) {
            best = Some((rect, area));
        }
    }
    best.expect("four placement candidates").0
}

// The logical corridor is tested against pointer events; it never owns a native window.
pub fn corridor_contains(a: Rect, b: Rect, x: f64, y: f64) -> bool {
    if a.contains(x, y) || b.contains(x, y) {
        return false;
    }
    if a.bottom() <= b.y || b.bottom() <= a.y {
        let (top, bottom) = if a.y < b.y { (a, b) } else { (b, a) };
        let height = bottom.y - top.bottom();
        if height <= 0.0 || y < top.bottom() || y > bottom.y {
            return false;
        }
        let t = (y - top.bottom()) / height;
        x >= top.x + (bottom.x - top.x) * t && x <= top.right() + (bottom.right() - top.right()) * t
    } else if a.right() <= b.x || b.right() <= a.x {
        let (left, right) = if a.x < b.x { (a, b) } else { (b, a) };
        let width = right.x - left.right();
        if width <= 0.0 || x < left.right() || x > right.x {
            return false;
        }
        let t = (x - left.right()) / width;
        y >= left.y + (right.y - left.y) * t
            && y <= left.bottom() + (right.bottom() - left.bottom()) * t
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_gap_release_and_resize_preserve_anchor_intent_at_destination_dpi() {
        for scale in [1.0, 1.25, 1.5, 2.0, 2.5] {
            for gap in [0, 8, 16, 48] {
                let work = Rect {
                    x: -1600.0,
                    y: -120.0,
                    width: 1600.0,
                    height: 1000.0,
                };
                let size = (200.0 * scale, 80.0 * scale);
                let policy = gap_constraints(work, size, scale, gap, [false; 4], false);
                let dropped = Rect {
                    x: work.right() - size.0,
                    y: work.bottom() - size.1,
                    width: size.0,
                    height: size.1,
                };
                let anchors = release_anchors(
                    dropped,
                    work,
                    policy.margins,
                    scale,
                    SnapAnchors::default(),
                    false,
                );
                assert_eq!(
                    anchors,
                    SnapAnchors {
                        horizontal: HorizontalAnchor::Right,
                        vertical: VerticalAnchor::Bottom
                    }
                );
                let rect = resolve_anchors(dropped, work, anchors, policy.margins, [0.0; 4], None);
                assert!((work.right() - rect.right() - f64::from(gap) * scale).abs() <= 1.0);
                assert!((work.bottom() - rect.bottom() - f64::from(gap) * scale).abs() <= 1.0);
                let larger = Rect {
                    width: rect.width + 35.0,
                    height: rect.height + 20.0,
                    ..rect
                };
                let resized =
                    resolve_anchors(larger, work, anchors, policy.margins, [0.0; 4], None);
                assert_eq!(resized.right(), rect.right());
                assert_eq!(resized.bottom(), rect.bottom());
            }
        }
    }
    #[test]
    fn attraction_band_includes_outer_inset_and_fine_positions_remain_free() {
        let work = Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        let rect = Rect {
            x: 60.0,
            y: 150.0,
            width: 100.0,
            height: 60.0,
        };
        let policy = gap_constraints(work, (100.0, 60.0), 1.0, 48, [false; 4], false);
        assert_eq!(
            release_anchors(
                rect,
                work,
                policy.margins,
                1.0,
                SnapAnchors::default(),
                false
            )
            .horizontal,
            HorizontalAnchor::Left
        );
        assert_eq!(
            release_anchors(
                Rect { x: 60.01, ..rect },
                work,
                policy.margins,
                1.0,
                SnapAnchors::default(),
                false
            )
            .horizontal,
            HorizontalAnchor::None
        );
        let free = resolve_anchors(
            Rect { x: 1.0, ..rect },
            work,
            SnapAnchors::default(),
            policy.margins,
            [0.0; 4],
            None,
        );
        assert_eq!(free.x, 1.0);
    }
    #[test]
    fn taskbar_safety_fit_and_oversized_content_are_explicit() {
        let work = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 80.0,
        };
        let p = gap_constraints(
            work,
            (90.0, 60.0),
            2.0,
            48,
            [false, false, false, true],
            false,
        );
        assert!(p.reduced_to_fit);
        assert!(p.margins[0] + p.margins[2] <= 10.0);
        assert!(p.margins[1] + p.margins[3] <= 20.0);
        let p = gap_constraints(
            work,
            (40.0, 30.0),
            1.0,
            0,
            [false, false, false, true],
            false,
        );
        assert_eq!(p.margins, [0.0, 0.0, 0.0, 8.0]);
        let p = gap_constraints(work, (200.0, 100.0), 1.0, 8, [true; 4], true);
        assert!(p.content_exceeds_work_area);
        assert!(p.taskbar_uncertain);
        assert_eq!(p.margins, [0.0; 4]);
    }
    #[test]
    fn legacy_inference_is_conservative_and_edge_has_one_horizontal_owner() {
        let work = Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        let rect = Rect {
            x: 0.5,
            y: 1.0,
            width: 100.0,
            height: 60.0,
        };
        assert_eq!(
            infer_legacy(rect, work, 1.0, false),
            SnapAnchors {
                horizontal: HorizontalAnchor::Left,
                vertical: VerticalAnchor::Top
            }
        );
        assert_eq!(
            infer_legacy(
                Rect {
                    x: 20.0,
                    y: 20.0,
                    ..rect
                },
                work,
                1.0,
                false
            ),
            SnapAnchors::default()
        );
        assert_eq!(
            infer_legacy(Rect { y: 8.0, ..rect }, work, 1.0, true),
            SnapAnchors {
                horizontal: HorizontalAnchor::None,
                vertical: VerticalAnchor::Top
            }
        );
    }
    #[test]
    fn opposing_attraction_ties_retain_prior_anchor_then_choose_left_top() {
        let work = Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        };
        let rect = Rect {
            x: 10.0,
            y: 10.0,
            width: 80.0,
            height: 80.0,
        };
        let prior = SnapAnchors {
            horizontal: HorizontalAnchor::Right,
            vertical: VerticalAnchor::Bottom,
        };
        assert_eq!(
            release_anchors(rect, work, [8.0; 4], 1.0, prior, false),
            prior
        );
        assert_eq!(
            release_anchors(rect, work, [8.0; 4], 1.0, SnapAnchors::default(), false),
            SnapAnchors {
                horizontal: HorizontalAnchor::Left,
                vertical: VerticalAnchor::Top
            }
        );
    }
    #[test]
    fn edge_drop_chooses_side_clamps_height_and_keeps_ties_at_destination_dpi() {
        for scale in [1.0, 1.25, 1.5, 2.5] {
            let work = Rect {
                x: -1200.0 * scale,
                y: 40.0 * scale,
                width: 1200.0 * scale,
                height: 900.0 * scale,
            };
            let size = (60.0 * scale, 184.0 * scale);
            for (x, expected_right) in [(work.x, false), (work.right() - size.0, true)] {
                for y in [work.y - 50.0, work.y + 150.0, work.bottom() + 50.0] {
                    let (docked, right) = dock_edge(
                        Rect {
                            x,
                            y,
                            width: size.0,
                            height: size.1,
                        },
                        work,
                        scale,
                        !expected_right,
                    );
                    assert_eq!(right, expected_right);
                    assert_eq!(docked.width, size.0);
                    assert_eq!(docked.height, size.1);
                    assert_eq!(
                        if right {
                            work.right() - docked.right()
                        } else {
                            docked.x - work.x
                        },
                        8.0 * scale
                    );
                    assert_eq!(
                        docked.y,
                        y.clamp(work.y + 8.0 * scale, work.bottom() - 8.0 * scale - size.1)
                    );
                    let placement =
                        Placement::from_physical(docked, work, scale, Some("drop-monitor".into()));
                    assert_eq!(placement.physical(work, scale, size), docked);
                    assert_eq!(placement.monitor.as_deref(), Some("drop-monitor"));
                }
            }
            for previous_right in [false, true] {
                let (_, right) = dock_edge(
                    Rect {
                        x: work.x + (work.width - size.0) / 2.0,
                        y: work.y,
                        width: size.0,
                        height: size.1,
                    },
                    work,
                    scale,
                    previous_right,
                );
                assert_eq!(right, previous_right);
            }
        }
    }
    #[test]
    fn drop_monitor_distance_handles_gaps_and_negative_coordinates() {
        let left = Rect {
            x: -1920.0,
            y: -200.0,
            width: 1920.0,
            height: 1080.0,
        };
        let right = Rect {
            x: 100.0,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
        };
        assert_eq!(point_distance(left, (-800.0, 300.0)), 0.0);
        assert_eq!(point_distance(right, (1500.0, 300.0)), 0.0);
        assert!(point_distance(left, (30.0, 300.0)) < point_distance(right, (30.0, 300.0)));
        assert!(point_distance(right, (90.0, 300.0)) < point_distance(left, (90.0, 300.0)));
    }
    #[test]
    fn hud_hover_centers_flips_and_clamps_at_fractional_dpi() {
        for scale in [1.0, 1.25, 1.5, 2.5] {
            let work = Rect {
                x: -1200.0 * scale,
                y: 40.0 * scale,
                width: 1200.0 * scale,
                height: 900.0 * scale,
            };
            for (trigger_width, panel_width) in [(400.0, 340.0), (300.0, 480.0)] {
                let trigger = Rect {
                    x: -700.0 * scale,
                    y: 500.0 * scale,
                    width: trigger_width * scale,
                    height: 50.0 * scale,
                };
                let panel = centered_panel(
                    trigger,
                    panel_width * scale,
                    200.0 * scale,
                    8.0 * scale,
                    work,
                );
                assert_eq!(panel.x + panel.width / 2.0, trigger.x + trigger.width / 2.0);
                assert_eq!(panel.bottom() + 8.0 * scale, trigger.y);
                for x in [work.x, work.right() - trigger.width] {
                    let top = Rect {
                        x,
                        y: work.y,
                        ..trigger
                    };
                    let panel =
                        centered_panel(top, panel_width * scale, 200.0 * scale, 8.0 * scale, work);
                    assert_eq!(panel, panel.clamp(work));
                    assert_eq!(panel.y, top.bottom() + 8.0 * scale);
                }
            }
        }
    }

    #[test]
    fn edge_hover_preserves_gap_and_pointer_corridor_on_both_sides() {
        let work = Rect {
            x: -1200.0,
            y: 0.0,
            width: 1200.0,
            height: 900.0,
        };
        for right in [false, true] {
            let trigger = Rect {
                x: if right { -80.0 } else { -1192.0 },
                y: 800.0,
                width: 72.0,
                height: 90.0,
            };
            let panel = anchored(trigger, 340.0, 330.0, 8.0, work, Some(right));
            let gap = if right {
                trigger.x - panel.right()
            } else {
                panel.x - trigger.right()
            };
            assert_eq!(gap, 8.0);
            assert_eq!(panel.bottom(), work.bottom());
            let x = if right {
                trigger.x - 4.0
            } else {
                trigger.right() + 4.0
            };
            assert!(corridor_contains(trigger, panel, x, 840.0));
        }
    }
    #[test]
    fn floating_detail_uses_each_available_side_without_overlap() {
        let work = Rect {
            x: 0.0,
            y: 0.0,
            width: 1000.0,
            height: 800.0,
        };
        for (trigger, size, expected) in [
            (
                Rect {
                    x: 400.0,
                    y: 500.0,
                    width: 100.0,
                    height: 100.0,
                },
                (200.0, 200.0),
                (300.0, 292.0),
            ),
            (
                Rect {
                    x: 400.0,
                    y: 0.0,
                    width: 100.0,
                    height: 100.0,
                },
                (200.0, 200.0),
                (300.0, 108.0),
            ),
            (
                Rect {
                    x: 0.0,
                    y: 300.0,
                    width: 100.0,
                    height: 200.0,
                },
                (300.0, 650.0),
                (108.0, 150.0),
            ),
            (
                Rect {
                    x: 900.0,
                    y: 300.0,
                    width: 100.0,
                    height: 200.0,
                },
                (300.0, 650.0),
                (592.0, 150.0),
            ),
        ] {
            let detail = around(trigger, size.0, size.1, 8.0, work);
            assert_eq!((detail.x, detail.y), expected);
            assert_eq!(overlap(detail, trigger), 0.0);
        }
    }

    #[test]
    fn floating_detail_clamps_on_negative_monitors_at_fractional_scale() {
        for scale in [1.0, 1.25, 1.5, 2.5] {
            let work = Rect {
                x: -1000.0 * scale,
                y: -100.0 * scale,
                width: 1000.0 * scale,
                height: 800.0 * scale,
            };
            for (x, y) in [
                (work.x, work.y),
                (-100.0 * scale, work.y),
                (work.x, 500.0 * scale),
                (-100.0 * scale, 500.0 * scale),
            ] {
                let trigger = Rect {
                    x,
                    y,
                    width: 100.0 * scale,
                    height: 200.0 * scale,
                };
                let detail = around(trigger, 300.0 * scale, 650.0 * scale, 8.0 * scale, work);
                assert_eq!(detail, detail.clamp(work));
                assert_eq!(overlap(detail, trigger), 0.0);
            }
            let oversized = around(work, 2000.0 * scale, 1000.0 * scale, 8.0 * scale, work);
            assert_eq!(oversized, work);
        }
    }

    #[test]
    fn floating_detail_minimizes_unavoidable_overlap() {
        let work = Rect {
            x: 0.0,
            y: 0.0,
            width: 400.0,
            height: 300.0,
        };
        let trigger = Rect {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 100.0,
        };
        let detail = around(trigger, 250.0, 250.0, 8.0, work);
        assert_eq!((detail.x, detail.y), (150.0, 50.0));
        assert_eq!(overlap(detail, trigger), 15_000.0);
    }
    #[test]
    fn content_size_preserves_client_pixels_at_fractional_dpi_with_and_without_shadows() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 2.5] {
            for frame in [(0, 0), (16, 8), (40, 20)] {
                for content in [(282.0, 327.0), (62.0, 278.0), (462.0, 47.0)] {
                    let outer = content_outer_size(content, scale, frame);
                    let client = (outer.0 - f64::from(frame.0), outer.1 - f64::from(frame.1));
                    assert!(client.0 >= content.0 * scale);
                    assert!(client.1 >= content.1 * scale);
                    assert!(client.0 - content.0 * scale < 1.0);
                    assert!(client.1 - content.1 * scale < 1.0);
                }
            }
        }
    }
    #[test]
    fn content_frame_is_included_in_work_area_clamping_and_edge_anchor() {
        let work = Rect {
            x: -1920.0,
            y: 40.0,
            width: 1920.0,
            height: 1040.0,
        };
        let size = content_outer_size((62.0, 278.0), 2.5, (40, 20));
        let placement = Placement {
            x: 1900.0,
            y: 1000.0,
            monitor: None,
        };
        let shell = placement.physical(work, 2.5, size);
        assert_eq!((shell.width, shell.height), (195.0, 715.0));
        assert_eq!(shell.right(), work.right());
        assert_eq!(shell.bottom(), work.bottom());
        let popup = anchored(shell, 480.0, 650.0, 0.0, work, Some(true));
        assert_eq!(popup.right(), shell.x);
        let oversized = placement.physical(
            work,
            2.5,
            content_outer_size((2000.0, 1000.0), 2.5, (40, 20)),
        );
        assert_eq!(oversized, work);
    }
    #[test]
    fn fullscreen_bounds_distinguish_monitor_coverage_from_maximized_work_area() {
        let monitor = Rect {
            x: -1920.0,
            y: 0.0,
            width: 1920.0,
            height: 1080.0,
        };
        assert!(monitor.covers_monitor(monitor));
        assert!(!Rect {
            height: 1040.0,
            ..monitor
        }
        .covers_monitor(monitor));
        assert!(Rect {
            x: -1919.0,
            y: 1.0,
            width: 1918.0,
            height: 1078.0
        }
        .covers_monitor(monitor));
    }
    #[test]
    fn negative_monitor_and_mixed_dpi_roundtrip() {
        let work = Rect {
            x: -2560.0,
            y: -200.0,
            width: 2560.0,
            height: 1400.0,
        };
        let p = Placement {
            monitor: Some("second".into()),
            x: 100.0,
            y: 120.0,
        };
        let rect = p.physical(work, 2.0, (600.0, 140.0));
        assert_eq!(rect.x, -2360.0);
        assert_eq!(rect.y, 40.0);
        let restored = Placement::from_physical(rect, work, 2.0, p.monitor);
        assert_eq!(restored.x, 100.0);
        assert_eq!(restored.y, 120.0);
    }
    #[test]
    fn popup_flips_and_clamps_whole_surface_including_oversized_content() {
        let work = Rect {
            x: 0.0,
            y: 40.0,
            width: 1200.0,
            height: 760.0,
        };
        let trigger = Rect {
            x: 1180.0,
            y: 40.0,
            width: 20.0,
            height: 20.0,
        };
        let popup = anchored(trigger, 260.0, 76.0, 8.0, work, None);
        assert_eq!(popup.y, 68.0);
        assert_eq!(popup.right(), 1200.0);
        let large = anchored(trigger, 2400.0, 1000.0, 8.0, work, None);
        assert_eq!(large, work);
    }
    #[test]
    fn corridor_is_only_the_connecting_trapezoid() {
        let a = Rect {
            x: 200.0,
            y: 100.0,
            width: 20.0,
            height: 20.0,
        };
        let b = Rect {
            x: 0.0,
            y: 0.0,
            width: 260.0,
            height: 92.0,
        };
        assert!(corridor_contains(a, b, 210.0, 96.0));
        assert!(!corridor_contains(a, b, 10.0, 99.0));
        assert!(!corridor_contains(a, b, 300.0, 96.0));
        assert!(!corridor_contains(a, b, 210.0, 110.0));
    }
    #[test]
    fn edge_popup_and_hover_share_side_anchoring_and_vertical_clamping() {
        let work = Rect {
            x: -1920.0,
            y: 40.0,
            width: 1920.0,
            height: 1040.0,
        };
        for right in [false, true] {
            let trigger = Rect {
                x: if right { -88.0 } else { -1912.0 },
                y: 900.0,
                width: 80.0,
                height: 140.0,
            };
            for (width, height) in [(340.0, 330.0), (480.0, 650.0)] {
                let panel = anchored(trigger, width, height, 0.0, work, Some(right));
                assert_eq!(
                    if right { panel.right() } else { panel.x },
                    if right { trigger.x } else { trigger.right() }
                );
                assert_eq!(panel.bottom(), work.bottom());
            }
        }
    }
}

/// Dimensions are physical pixels, including native window frame.
pub fn dashboard_size(natural: (f64, f64), work: Rect, manual: bool) -> (f64, f64) {
    (
        natural.0.min(work.width),
        natural
            .1
            .min((work.height * if manual { 1.0 } else { 0.85 }).floor()),
    )
}

#[cfg(test)]
mod dashboard_tests {
    use super::*;
    #[test]
    fn natural_height_is_capped_in_physical_pixels_including_native_frame() {
        for scale in [1.0, 1.5, 2.5] {
            let work = Rect {
                x: 0.0,
                y: 40.0,
                width: 1920.0 * scale,
                height: 1000.0 * scale,
            };
            for height in [100.0, 849.0, 850.0, 851.0, 2400.0] {
                let size = dashboard_size((620.0 * scale, height * scale), work, false);
                assert_eq!(size.1, (height * scale).min((850.0 * scale).floor()));
            }
        }
        let small = Rect {
            x: 0.0,
            y: 0.0,
            width: 640.0,
            height: 320.0,
        };
        assert_eq!(dashboard_size((620.0, 640.0), small, false), (620.0, 272.0));
        assert_eq!(dashboard_size((700.0, 640.0), small, true), (640.0, 320.0));
    }
}
