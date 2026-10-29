use crate::{
    motion::Spring,
    store::{DockAnchor, DockEdge},
};
use windows::Win32::Foundation::{POINT, RECT};

pub const ICON_SIZE: f32 = 48.0;
pub const HIDE_DELAY_MS: u32 = 320;
const SNAP_DISTANCE: f32 = 18.0;

pub fn contains(bounds: RECT, point: POINT) -> bool {
    point.x >= bounds.left
        && point.x < bounds.right
        && point.y >= bounds.top
        && point.y < bounds.bottom
}

pub fn detect(bounds: RECT, work: RECT, scale: f32) -> Option<DockAnchor> {
    let (distance, edge) = [
        (
            i64::from(bounds.left) - i64::from(work.left),
            DockEdge::Left,
        ),
        (
            i64::from(work.right) - i64::from(bounds.right),
            DockEdge::Right,
        ),
        (i64::from(bounds.top) - i64::from(work.top), DockEdge::Top),
        (
            i64::from(work.bottom) - i64::from(bounds.bottom),
            DockEdge::Bottom,
        ),
    ]
    .into_iter()
    .min_by_key(|(distance, _)| *distance)?;
    if distance as f32 > SNAP_DISTANCE * scale {
        return None;
    }
    let (center, start, length) = match edge {
        DockEdge::Left | DockEdge::Right => (
            (bounds.top as f32 + bounds.bottom as f32) / 2.0,
            work.top,
            work.bottom - work.top,
        ),
        DockEdge::Top | DockEdge::Bottom => (
            (bounds.left as f32 + bounds.right as f32) / 2.0,
            work.left,
            work.right - work.left,
        ),
    };
    let icon = (ICON_SIZE * scale).round();
    let position =
        ((center - start as f32 - icon / 2.0) / (length as f32 - icon).max(1.0)).clamp(0.0, 1.0);
    Some(DockAnchor { edge, position })
}

#[derive(Clone, Copy)]
pub struct Layout {
    pub icon: RECT,
    pub expanded: RECT,
}

impl Layout {
    pub fn new(anchor: DockAnchor, width: i32, height: i32, scale: f32, work: RECT) -> Self {
        let work_width = (work.right - work.left).max(1);
        let work_height = (work.bottom - work.top).max(1);
        let icon_width = ((ICON_SIZE * scale).round() as i32).clamp(1, work_width);
        let icon_height = ((ICON_SIZE * scale).round() as i32).clamp(1, work_height);
        let position = anchor.position.clamp(0.0, 1.0);
        let (x, y) = match anchor.edge {
            DockEdge::Left => (
                work.left,
                work.top + ((work_height - icon_height) as f32 * position).round() as i32,
            ),
            DockEdge::Right => (
                work.right - icon_width,
                work.top + ((work_height - icon_height) as f32 * position).round() as i32,
            ),
            DockEdge::Top => (
                work.left + ((work_width - icon_width) as f32 * position).round() as i32,
                work.top,
            ),
            DockEdge::Bottom => (
                work.left + ((work_width - icon_width) as f32 * position).round() as i32,
                work.bottom - icon_height,
            ),
        };
        let icon = RECT {
            left: x,
            top: y,
            right: x + icon_width,
            bottom: y + icon_height,
        };
        let width = ((width as f32 * scale).round() as i32).clamp(icon_width, work_width);
        let height = ((height as f32 * scale).round() as i32).clamp(icon_height, work_height);
        let x = x.clamp(work.left, work.right - width);
        let y = y.clamp(work.top, work.bottom - height);
        Self {
            icon,
            expanded: RECT {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            },
        }
    }
}

pub struct Dock {
    pub layout: Layout,
    pub reveal: Spring,
    pub open: bool,
    geometry: [Spring; 4],
    hover: HoverGate,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum HoverGate {
    WaitForExit,
    IconReady,
    Preview,
}
impl Dock {
    pub fn new(layout: Layout) -> Self {
        let icon = layout.icon;
        Self {
            layout,
            reveal: Spring::new(0.0),
            open: false,
            geometry: [icon.left, icon.top, icon.right, icon.bottom]
                .map(|value| Spring::new(value as f32)),
            hover: HoverGate::IconReady,
        }
    }
    pub fn from_expanded(layout: Layout, bounds: RECT, animate: bool) -> Self {
        let mut dock = Self::new(layout);
        dock.reveal.snap(1.0);
        for (spring, value) in
            dock.geometry
                .iter_mut()
                .zip([bounds.left, bounds.top, bounds.right, bounds.bottom])
        {
            spring.snap(value as f32);
        }
        dock.set_open(false, animate);
        dock.hover = HoverGate::WaitForExit;
        dock
    }
    pub fn set_open(&mut self, open: bool, animate: bool) {
        self.open = open;
        if open {
            self.hover = HoverGate::Preview;
        }
        let target = if open { 1.0 } else { 0.0 };
        if animate {
            self.reveal.set(target);
        } else {
            self.reveal.snap(target);
        }
        let bounds = if open {
            self.layout.expanded
        } else {
            self.layout.icon
        };
        for (spring, value) in
            self.geometry
                .iter_mut()
                .zip([bounds.left, bounds.top, bounds.right, bounds.bottom])
        {
            if animate {
                spring.set(value as f32);
            } else {
                spring.snap(value as f32);
            }
        }
        self.finish_hover();
    }
    pub fn step(&mut self, dt: f32) {
        self.reveal.step(dt);
        for spring in &mut self.geometry {
            spring.step(dt);
        }
        self.finish_hover();
    }
    fn finish_hover(&mut self) {
        if !self.open && !self.active() && self.hover == HoverGate::Preview {
            self.hover = HoverGate::IconReady;
        }
    }
    pub fn observe_pointer(&mut self, point: POINT) {
        if self.hover == HoverGate::WaitForExit && !contains(self.layout.icon, point) {
            self.hover = HoverGate::IconReady;
        }
    }
    pub fn wants_hover(&mut self, point: POINT, over_window: bool) -> bool {
        self.observe_pointer(point);
        // The final icon is stationary while the original panel is shrinking.
        // Old client/nonclient moves must never turn that shrink into a preview.
        over_window
            && (self.open
                || (self.hover == HoverGate::Preview && self.active())
                || (self.hover == HoverGate::IconReady && contains(self.layout.icon, point)))
    }
    pub fn active(&self) -> bool {
        self.reveal.active() || self.geometry.iter().any(Spring::active)
    }
    pub fn bounds(&self) -> RECT {
        let [left, top, right, bottom] = self.geometry.map(|spring| spring.value.round() as i32);
        RECT {
            left,
            top,
            right: right.max(left + 1),
            bottom: bottom.max(top + 1),
        }
    }
    pub fn interactive(&self) -> bool {
        self.open && !self.active()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const WORK: RECT = RECT {
        left: -1920,
        top: -80,
        right: 0,
        bottom: 960,
    };
    fn inside(rect: RECT, work: RECT) -> bool {
        rect.left >= work.left
            && rect.right <= work.right
            && rect.top >= work.top
            && rect.bottom <= work.bottom
    }
    #[test]
    fn fresh_docking_needs_exit_then_icon_entry_but_normal_preview_can_reverse() {
        for edge in [
            DockEdge::Left,
            DockEdge::Right,
            DockEdge::Top,
            DockEdge::Bottom,
        ] {
            for position in [0.0, 0.5, 1.0] {
                let layout = Layout::new(DockAnchor { edge, position }, 316, 366, 1.5, WORK);
                let icon = POINT {
                    x: (layout.icon.left + layout.icon.right) / 2,
                    y: (layout.icon.top + layout.icon.bottom) / 2,
                };
                let body = POINT {
                    x: (layout.expanded.left + layout.expanded.right) / 2,
                    y: (layout.expanded.top + layout.expanded.bottom) / 2,
                };
                assert!(!contains(layout.icon, body));
                let mut dock = Dock::from_expanded(layout, layout.expanded, true);
                assert!(
                    !dock.wants_hover(icon, true),
                    "the drag's remaining cursor position must not open a fresh dock"
                );
                for _ in 0..150 {
                    dock.step(0.016);
                }
                assert!(
                    !dock.wants_hover(icon, true),
                    "finishing the shrink must not arm stationary hover"
                );
                assert!(
                    !dock.wants_hover(body, true),
                    "the old panel body is not an icon hover target"
                );
                assert!(
                    !dock.wants_hover(icon, false),
                    "a stale mouse move outside the actual window cannot open it"
                );
                assert!(dock.wants_hover(icon, true));
                dock.set_open(true, false);
                dock.set_open(false, true);
                assert!(
                    dock.wants_hover(body, true),
                    "deliberate reentry during normal preview closing should remain responsive"
                );
                assert!(!dock.wants_hover(body, false));
            }
        }
        let mut restarted = Dock::new(Layout::new(
            DockAnchor {
                edge: DockEdge::Left,
                position: 0.5,
            },
            316,
            366,
            1.5,
            WORK,
        ));
        let icon = restarted.layout.icon;
        assert!(
            restarted.wants_hover(
                POINT {
                    x: icon.left + 10,
                    y: icon.top + 10
                },
                true
            ),
            "restored icons can hover immediately"
        );
    }
    #[test]
    fn four_edges_and_overshoot_snap_but_center_does_not() {
        for (bounds, edge) in [
            (
                RECT {
                    left: -1940,
                    top: 200,
                    right: -1466,
                    bottom: 749,
                },
                DockEdge::Left,
            ),
            (
                RECT {
                    left: -450,
                    top: 200,
                    right: 24,
                    bottom: 749,
                },
                DockEdge::Right,
            ),
            (
                RECT {
                    left: -1200,
                    top: -100,
                    right: -726,
                    bottom: 449,
                },
                DockEdge::Top,
            ),
            (
                RECT {
                    left: -1200,
                    top: 450,
                    right: -726,
                    bottom: 999,
                },
                DockEdge::Bottom,
            ),
        ] {
            let anchor = detect(bounds, WORK, 1.5).unwrap();
            assert_eq!(anchor.edge, edge);
            assert!((0.0..=1.0).contains(&anchor.position));
        }
        assert!(
            detect(
                RECT {
                    left: -1200,
                    top: 200,
                    right: -726,
                    bottom: 749
                },
                WORK,
                1.5
            )
            .is_none()
        );
        assert!(
            detect(
                RECT {
                    left: -1894,
                    top: 200,
                    right: -1420,
                    bottom: 749
                },
                WORK,
                1.5
            )
            .is_some()
        );
        assert!(
            detect(
                RECT {
                    left: -1892,
                    top: 200,
                    right: -1418,
                    bottom: 749
                },
                WORK,
                1.5
            )
            .is_none()
        );
    }
    #[test]
    fn icon_and_hover_content_stay_visible_at_all_edges_and_dpi() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for edge in [
                DockEdge::Left,
                DockEdge::Right,
                DockEdge::Top,
                DockEdge::Bottom,
            ] {
                for position in [0.0, 0.5, 1.0] {
                    let layout = Layout::new(DockAnchor { edge, position }, 316, 366, scale, WORK);
                    assert_eq!(
                        layout.icon.right - layout.icon.left,
                        (ICON_SIZE * scale).round() as i32
                    );
                    assert_eq!(
                        layout.expanded.bottom - layout.expanded.top,
                        (366.0 * scale).round() as i32
                    );
                    let mut dock = Dock::new(layout);
                    dock.set_open(true, true);
                    for _ in 0..100 {
                        let bounds = dock.bounds();
                        assert!(inside(bounds, WORK));
                        assert!(
                            inside(layout.icon, bounds),
                            "hovered icon must remain under the pointer throughout expansion"
                        );
                        dock.step(0.016);
                    }
                    assert!(!dock.active());
                }
            }
        }
    }
    #[test]
    fn interrupted_hover_reverses_smoothly_and_eventually_stops() {
        let mut dock = Dock::new(Layout::new(
            DockAnchor {
                edge: DockEdge::Right,
                position: 0.5,
            },
            316,
            366,
            1.5,
            WORK,
        ));
        dock.set_open(true, true);
        for _ in 0..5 {
            dock.step(0.016);
        }
        let before = dock.reveal.value;
        dock.set_open(false, true);
        assert_eq!(dock.reveal.value, before);
        for _ in 0..100 {
            dock.step(0.016);
        }
        assert_eq!(dock.reveal.value, 0.0);
        assert!(!dock.active());
        dock.set_open(true, false);
        assert!(dock.interactive());
    }
    #[test]
    fn initial_docking_and_hover_reversal_do_not_jump_the_window() {
        let layout = Layout::new(
            DockAnchor {
                edge: DockEdge::Left,
                position: 0.5,
            },
            316,
            366,
            1.5,
            WORK,
        );
        let start = RECT {
            left: WORK.left - 100,
            top: 100,
            right: WORK.left + 374,
            bottom: 649,
        };
        let mut dock = Dock::from_expanded(layout, start, true);
        assert_eq!(dock.bounds(), start);
        for _ in 0..5 {
            dock.step(0.016);
        }
        let before = dock.bounds();
        dock.set_open(true, true);
        assert_eq!(dock.bounds(), before);
        for _ in 0..100 {
            dock.step(0.016);
        }
        assert_eq!(dock.bounds(), layout.expanded);
        assert!(!dock.active());
    }
    #[test]
    fn geometry_stops_on_far_negative_monitor_coordinates() {
        let work = RECT {
            left: -32000,
            top: -12000,
            right: -28000,
            bottom: -10000,
        };
        for edge in [
            DockEdge::Left,
            DockEdge::Right,
            DockEdge::Top,
            DockEdge::Bottom,
        ] {
            let layout = Layout::new(
                DockAnchor {
                    edge,
                    position: 0.5,
                },
                316,
                366,
                1.5,
                work,
            );
            let mut dock = Dock::new(layout);
            for open in [true, false] {
                dock.set_open(open, true);
                for _ in 0..150 {
                    dock.step(0.016);
                }
                assert!(
                    !dock.active(),
                    "geometry must stop drawing at large desktop coordinates"
                );
            }
        }
    }
}
