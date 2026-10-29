use crate::{
    core::{Core, WM_REFRESH},
    motion::Spring,
    render::{Painter, color, rect},
    statuses::StatusDefinition,
};
use anyhow::Result;
use std::{sync::Arc, time::Instant};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Controls::WM_MOUSELEAVE, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::w,
};

const WIDTH: f32 = 244.0;
const HEADER: f32 = 42.0;
const ROW: f32 = 38.0;
const TIMER: usize = 81;
const DESTROY: u32 = WM_APP + 81;

pub fn popup_bounds(anchor: RECT, work: RECT, scale: f32, count: usize) -> RECT {
    let width = ((WIDTH * scale).round() as i32).clamp(1, (work.right - work.left).max(1));
    let height = (((HEADER + count.clamp(1, 7) as f32 * ROW + 10.0) * scale).round() as i32)
        .clamp(1, (work.bottom - work.top).max(1));
    let x = (anchor.right - width).clamp(work.left, (work.right - width).max(work.left));
    let gap = (6.0 * scale).round() as i32;
    let below = anchor.bottom + gap;
    let y = if below + height <= work.bottom {
        below
    } else {
        anchor.top - gap - height
    }
    .clamp(work.top, (work.bottom - height).max(work.top));
    RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    }
}
fn row_at(x: f32, y: f32, width: f32, height: f32, scroll: usize, count: usize) -> Option<usize> {
    if x < 10.0 || x >= width - 10.0 || y < HEADER || y >= height - 10.0 {
        return None;
    }
    let index = scroll + ((y - HEADER) / ROW) as usize;
    (index < count).then_some(index)
}
struct Picker {
    attached: bool,
    hwnd: HWND,
    core: Arc<Core>,
    painter: Painter,
    todo: i64,
    version: i64,
    definitions: Vec<StatusDefinition>,
    selected: String,
    focus: usize,
    scroll: usize,
    hover: Option<usize>,
    highlights: Vec<Spring>,
    reveal: Spring,
    last: Instant,
    timer: bool,
    closing: bool,
    scale: f32,
    dark: bool,
    bounds: RECT,
    error: String,
    enabled: bool,
}
impl Picker {
    fn visible_rows(&self) -> usize {
        (((self.bounds.bottom - self.bounds.top) as f32 / self.scale - HEADER - 10.0) / ROW)
            .floor()
            .max(1.0) as usize
    }
    fn active(&self) -> bool {
        self.reveal.active() || self.highlights.iter().any(Spring::active)
    }
    unsafe fn schedule(&mut self) {
        unsafe {
            if self.active() && !self.timer {
                self.timer = true;
                self.last = Instant::now();
                SetTimer(Some(self.hwnd), TIMER, 16, None);
            } else if !self.active() && self.timer {
                let _ = KillTimer(Some(self.hwnd), TIMER);
                self.timer = false;
            }
        }
    }
    unsafe fn close(&mut self) {
        unsafe {
            if !self.closing {
                self.closing = true;
                if self.enabled {
                    self.reveal.set(0.0);
                    self.schedule();
                } else {
                    self.reveal.snap(0.0);
                    let _ = PostMessageW(Some(self.hwnd), DESTROY, WPARAM(0), LPARAM(0));
                }
            }
        }
    }
    unsafe fn paint(&mut self) {
        unsafe {
            let width = self.bounds.right - self.bounds.left;
            let height = self.bounds.bottom - self.bounds.top;
            if self.painter.begin(width, height, self.scale).is_ok() {
                self.painter.group_opacity(self.reveal.value);
                draw(
                    &self.painter,
                    View {
                        definitions: &self.definitions,
                        selected: &self.selected,
                        highlights: &self.highlights,
                        scroll: self.scroll,
                        width: width as f32 / self.scale,
                        height: height as f32 / self.scale,
                        dark: self.dark,
                        error: &self.error,
                    },
                );
                self.painter.group_opacity(1.0);
                let _ = self.painter.end(self.hwnd);
            }
        }
    }
    unsafe fn highlight(&mut self, index: Option<usize>) {
        unsafe {
            self.hover = index;
            for (i, spring) in self.highlights.iter_mut().enumerate() {
                let target = if Some(i) == index { 1.0 } else { 0.0 };
                if self.enabled {
                    spring.set(target);
                } else {
                    spring.snap(target);
                }
            }
            self.schedule();
            self.paint();
        }
    }
    unsafe fn ensure_focus(&mut self) {
        unsafe {
            if self.focus < self.scroll {
                self.scroll = self.focus;
            }
            if self.focus >= self.scroll + self.visible_rows() {
                self.scroll = self.focus + 1 - self.visible_rows();
            }
            self.highlight(Some(self.focus));
            self.paint();
        }
    }
    unsafe fn select(&mut self, index: usize) {
        unsafe {
            if self.closing {
                return;
            }
            let Some(definition) = self.definitions.get(index) else {
                return;
            };
            if definition.id == self.selected {
                self.close();
                return;
            }
            match self.core.update_with_status(
                self.todo,
                None,
                None,
                Some(&definition.id),
                Some(self.version),
            ) {
                Ok(_) => self.close(),
                Err(_) => {
                    self.error = "状态已变化，请重新选择".into();
                    if let Ok(todo) = self.core.get(self.todo) {
                        self.version = todo.version;
                        self.selected = todo.status_id;
                    } else {
                        self.close();
                        return;
                    }
                    if let Ok(definitions) = self.core.statuses() {
                        self.definitions = definitions;
                        self.highlights = vec![Spring::new(0.0); self.definitions.len()];
                    }
                    self.focus = self.focus.min(self.definitions.len().saturating_sub(1));
                    self.ensure_focus();
                }
            }
        }
    }
    unsafe fn animate(&mut self) {
        unsafe {
            let now = Instant::now();
            let dt = now.duration_since(self.last).as_secs_f32();
            self.last = now;
            self.reveal.step(dt);
            for spring in &mut self.highlights {
                spring.step(dt);
            }
            let y = self.bounds.top + ((1.0 - self.reveal.value) * 5.0 * self.scale).round() as i32;
            let _ = SetWindowPos(
                self.hwnd,
                None,
                self.bounds.left,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            self.paint();
            self.schedule();
            if self.closing && !self.active() {
                let _ = PostMessageW(Some(self.hwnd), DESTROY, WPARAM(0), LPARAM(0));
            }
        }
    }
}
struct View<'a> {
    definitions: &'a [StatusDefinition],
    selected: &'a str,
    highlights: &'a [Spring],
    scroll: usize,
    width: f32,
    height: f32,
    dark: bool,
    error: &'a str,
}
unsafe fn draw(p: &Painter, view: View<'_>) {
    unsafe {
        let View {
            definitions,
            selected,
            highlights,
            scroll,
            width,
            height,
            dark,
            error,
        } = view;
        let ink = if dark {
            color(0.94, 0.96, 1.0, 1.0)
        } else {
            color(0.08, 0.14, 0.22, 1.0)
        };
        let muted = if dark {
            color(0.68, 0.76, 0.86, 1.0)
        } else {
            color(0.36, 0.44, 0.54, 1.0)
        };
        p.fill(
            rect(2.0, 4.0, width - 4.0, height - 5.0),
            18.0,
            color(0.03, 0.08, 0.15, 0.15),
        );
        p.fill(
            rect(2.0, 2.0, width - 4.0, height - 6.0),
            17.0,
            if dark {
                color(0.10, 0.14, 0.20, 0.98)
            } else {
                color(0.96, 0.98, 1.0, 0.98)
            },
        );
        p.border(
            rect(2.6, 2.6, width - 5.2, height - 7.2),
            16.4,
            color(1.0, 1.0, 1.0, if dark { 0.20 } else { 0.93 }),
            1.2,
        );
        p.text(
            if error.is_empty() {
                "切换状态"
            } else {
                error
            },
            rect(18.0, 13.0, width - 36.0, 24.0),
            0,
            if error.is_empty() {
                muted
            } else {
                color(0.8, 0.25, 0.28, 1.0)
            },
        );
        p.clip(rect(8.0, HEADER, width - 16.0, height - HEADER - 10.0));
        for (index, definition) in definitions.iter().enumerate().skip(scroll) {
            let y = HEADER + (index - scroll) as f32 * ROW;
            if y >= height - 10.0 {
                break;
            }
            let active = definition.id == selected;
            let (r, g, b) = definition.color.rgb(dark);
            let hover = highlights
                .get(index)
                .map(|value| value.value)
                .unwrap_or(0.0);
            p.fill(
                rect(10.0, y, width - 20.0, 34.0),
                10.0,
                color(r, g, b, if active { 0.12 } else { 0.0 } + hover * 0.08),
            );
            p.circle(26.0, y + 17.0, 4.0, color(r, g, b, 1.0), true);
            p.text(
                &definition.name,
                rect(42.0, y + 6.0, width - 82.0, 26.0),
                1,
                ink,
            );
            if active {
                p.line(
                    width - 29.0,
                    y + 17.0,
                    width - 25.0,
                    y + 21.0,
                    color(r, g, b, 1.0),
                    1.7,
                );
                p.line(
                    width - 25.0,
                    y + 21.0,
                    width - 18.0,
                    y + 13.0,
                    color(r, g, b, 1.0),
                    1.7,
                );
            }
        }
        p.unclip();
        let visible = ((height - HEADER - 10.0) / ROW).floor().max(1.0) as usize;
        if definitions.len() > visible {
            let track = height - HEADER - 18.0;
            let thumb = (track * visible as f32 / definitions.len() as f32).max(20.0);
            let y = HEADER
                + 4.0
                + scroll as f32 / (definitions.len() - visible) as f32 * (track - thumb);
            p.fill(
                rect(width - 7.0, y, 2.0, thumb),
                1.0,
                color(muted.r, muted.g, muted.b, 0.30),
            );
        }
    }
}

pub unsafe fn open(
    owner: HWND,
    core: Arc<Core>,
    todo_id: i64,
    anchor: RECT,
    scale: f32,
    dark: bool,
) -> Result<HWND> {
    unsafe { create(owner, core, todo_id, anchor, scale, dark, true) }
}
unsafe fn create(
    owner: HWND,
    core: Arc<Core>,
    todo_id: i64,
    anchor: RECT,
    scale: f32,
    dark: bool,
    show: bool,
) -> Result<HWND> {
    unsafe {
        let todo = core.get(todo_id)?;
        let definitions = core.statuses()?;
        let focus = definitions
            .iter()
            .position(|definition| definition.id == todo.status_id)
            .unwrap_or(0);
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(
            MonitorFromRect(&anchor, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        );
        let bounds = popup_bounds(anchor, monitor.rcWork, scale, definitions.len());
        let instance = GetModuleHandleW(None)?;
        let class = w!("GlassTodo.StatusPicker");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        });
        let highlights = vec![Spring::new(0.0); definitions.len()];
        let enabled = crate::ui::animations_enabled();
        let mut reveal = Spring::new(if enabled { 0.0 } else { 1.0 });
        reveal.set(1.0);
        let ptr = Box::into_raw(Box::new(Picker {
            attached: false,
            hwnd: HWND::default(),
            core,
            painter: Painter::new()?,
            todo: todo_id,
            version: todo.version,
            definitions,
            selected: todo.status_id,
            focus,
            scroll: 0,
            hover: None,
            highlights,
            reveal,
            last: Instant::now(),
            timer: false,
            closing: false,
            scale,
            dark,
            bounds,
            error: String::new(),
            enabled,
        }));
        let hwnd = match CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW,
            class,
            w!("切换状态"),
            WS_POPUP,
            bounds.left,
            bounds.top,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            Some(owner),
            None,
            Some(instance.into()),
            Some(ptr.cast()),
        ) {
            Ok(hwnd) => hwnd,
            Err(error) => {
                drop(Box::from_raw(ptr));
                return Err(error.into());
            }
        };
        (*ptr).attached = true;
        (*ptr).ensure_focus();
        if show {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
        }
        (*ptr).schedule();
        Ok(hwnd)
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let cs = &*(l.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            (*(cs.lpCreateParams as *mut Picker)).hwnd = hwnd;
            return DefWindowProcW(hwnd, msg, w, l);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Picker;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, w, l);
        }
        match msg {
            WM_PAINT => {
                (*ptr).paint();
                let _ = ValidateRect(Some(hwnd), None);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_MOUSEMOVE => {
                let index = row_at(
                    l.0 as u16 as i16 as f32 / (*ptr).scale,
                    (l.0 >> 16) as u16 as i16 as f32 / (*ptr).scale,
                    ((*ptr).bounds.right - (*ptr).bounds.left) as f32 / (*ptr).scale,
                    ((*ptr).bounds.bottom - (*ptr).bounds.top) as f32 / (*ptr).scale,
                    (*ptr).scroll,
                    (*ptr).definitions.len(),
                );
                (*ptr).highlight(index);
                let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                });
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                (*ptr).highlight(None);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                if let Some(index) = row_at(
                    l.0 as u16 as i16 as f32 / (*ptr).scale,
                    (l.0 >> 16) as u16 as i16 as f32 / (*ptr).scale,
                    ((*ptr).bounds.right - (*ptr).bounds.left) as f32 / (*ptr).scale,
                    ((*ptr).bounds.bottom - (*ptr).bounds.top) as f32 / (*ptr).scale,
                    (*ptr).scroll,
                    (*ptr).definitions.len(),
                ) {
                    (*ptr).select(index);
                }
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = (w.0 >> 16) as u16 as i16;
                let max = (*ptr)
                    .definitions
                    .len()
                    .saturating_sub((*ptr).visible_rows());
                (*ptr).scroll = if delta > 0 {
                    (*ptr).scroll.saturating_sub(2)
                } else {
                    (*ptr).scroll.saturating_add(2).min(max)
                };
                (*ptr).paint();
                LRESULT(0)
            }
            WM_KEYDOWN => {
                let key = w.0 as u16;
                if key == VK_ESCAPE.0 {
                    (*ptr).close();
                } else if key == VK_RETURN.0 {
                    (*ptr).select((*ptr).focus);
                } else if key == VK_UP.0 || key == VK_DOWN.0 || key == VK_HOME.0 || key == VK_END.0
                {
                    let count = (*ptr).definitions.len();
                    (*ptr).focus = if key == VK_HOME.0 {
                        0
                    } else if key == VK_END.0 {
                        count.saturating_sub(1)
                    } else if key == VK_UP.0 {
                        ((*ptr).focus + count - 1) % count
                    } else {
                        ((*ptr).focus + 1) % count
                    };
                    (*ptr).ensure_focus();
                }
                LRESULT(0)
            }
            WM_ACTIVATE if w.0 as u16 == WA_INACTIVE as u16 => {
                (*ptr).close();
                LRESULT(0)
            }
            WM_CLOSE => {
                (*ptr).close();
                LRESULT(0)
            }
            WM_TIMER if w.0 == TIMER => {
                (*ptr).animate();
                LRESULT(0)
            }
            WM_REFRESH => {
                if (*ptr).core.get((*ptr).todo).is_err() {
                    (*ptr).close();
                }
                LRESULT(0)
            }
            DESTROY => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                if (*ptr).attached {
                    let picker = Box::from_raw(ptr);
                    let _ = KillTimer(Some(hwnd), TIMER);
                    picker.core.notify();
                }
                DefWindowProcW(hwnd, msg, w, l)
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{statuses::StatusColor, store::Status};
    #[test]
    fn picker_geometry_stays_in_work_area_and_hits_only_status_rows() {
        let work = RECT {
            left: -1920,
            top: -180,
            right: 0,
            bottom: 900,
        };
        for scale in [1.0, 1.5, 2.0] {
            for (x, y) in [(-1920, -180), (-20, -180), (-1920, 880), (-20, 880)] {
                let bounds = popup_bounds(
                    RECT {
                        left: x,
                        top: y,
                        right: x + 20,
                        bottom: y + 20,
                    },
                    work,
                    scale,
                    24,
                );
                assert!(
                    bounds.left >= work.left
                        && bounds.top >= work.top
                        && bounds.right <= work.right
                        && bounds.bottom <= work.bottom
                );
                assert!((bounds.bottom - bounds.top) as f32 <= 318.0 * scale);
            }
        }
        assert_eq!(row_at(30.0, 25.0, WIDTH, 242.0, 0, 5), None);
        assert_eq!(
            row_at(30.0, HEADER + ROW + 5.0, WIDTH, 318.0, 7, 24),
            Some(8)
        );
        assert_eq!(row_at(30.0, 240.0, WIDTH, 242.0, 0, 5), None);
        assert_eq!(row_at(3.0, 55.0, WIDTH, 242.0, 0, 5), None);
    }
    #[test]
    fn native_picker_keeps_version_guards_and_animation_timers_stop() {
        unsafe {
            let path =
                std::env::temp_dir().join(format!("glass-picker-{}.sqlite3", std::process::id()));
            let core = Arc::new(Core::new(&path).unwrap());
            let definition = core
                .create_status("发布测试", Status::InProgress, StatusColor::Purple)
                .unwrap();
            let todo = core.create("private picker fixture").unwrap();
            let owner = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("private picker owner"),
                WS_POPUP,
                -32000,
                -32000,
                100,
                100,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let anchor = RECT {
                left: 50,
                top: 50,
                right: 100,
                bottom: 80,
            };
            let hwnd = create(owner, core.clone(), todo.id, anchor, 1.0, false, false).unwrap();
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Picker;
            assert!(!IsWindowVisible(hwnd).as_bool());
            let index = (*ptr)
                .definitions
                .iter()
                .position(|value| value.id == definition.id)
                .unwrap();
            core.update(
                todo.id,
                Some("newer agent content"),
                None,
                Some(todo.version),
            )
            .unwrap();
            (*ptr).select(index);
            assert!(!(*ptr).closing);
            let picker = &*ptr;
            assert!(!picker.error.is_empty());
            assert_eq!(core.get(todo.id).unwrap().status_id, "pending");
            assert_eq!(core.get(todo.id).unwrap().content, "newer agent content");
            (*ptr).select(index);
            let after = core.get(todo.id).unwrap();
            assert_eq!(after.status_id, definition.id);
            assert_eq!(after.content, "newer agent content");
            assert!((*ptr).closing);
            for _ in 0..100 {
                (*ptr).last = Instant::now() - std::time::Duration::from_millis(16);
                (*ptr).animate();
            }
            assert!(!(*ptr).active());
            assert!(!(*ptr).timer);
            DestroyWindow(hwnd).unwrap();
            let hwnd = create(owner, core.clone(), todo.id, anchor, 1.0, false, false).unwrap();
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Picker;
            let index = (*ptr)
                .definitions
                .iter()
                .position(|value| value.id == definition.id)
                .unwrap();
            (*ptr).enabled = false;
            (*ptr).reveal.snap(1.0);
            (*ptr).select(index);
            assert_eq!(core.get(todo.id).unwrap().version, after.version);
            assert_eq!((*ptr).reveal.value, 0.0);
            DestroyWindow(hwnd).unwrap();
            DestroyWindow(owner).unwrap();
            drop(core);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn picker_renders_five_states_in_both_themes() {
        unsafe {
            let mut definitions: Vec<_> = [Status::Pending, Status::InProgress, Status::Done]
                .into_iter()
                .map(StatusDefinition::builtin)
                .collect();
            for (id, name, color) in [
                ("custom_1", "发布测试", StatusColor::Purple),
                ("custom_2", "发布预处理", StatusColor::Amber),
            ] {
                definitions.push(StatusDefinition {
                    id: id.into(),
                    name: name.into(),
                    kind: Status::InProgress,
                    color,
                    builtin: false,
                    version: 1,
                });
            }
            let highlights = vec![Spring::new(0.0); definitions.len()];
            let mut painter = Painter::new().unwrap();
            let dir = std::path::Path::new(".scratch/qa-status-picker");
            std::fs::create_dir_all(dir).unwrap();
            for dark in [false, true] {
                painter.begin(366, 363, 1.5).unwrap();
                draw(
                    &painter,
                    View {
                        definitions: &definitions,
                        selected: "custom_1",
                        highlights: &highlights,
                        scroll: 0,
                        width: WIDTH,
                        height: 242.0,
                        dark,
                        error: "",
                    },
                );
                let pixels = painter.test_pixels().unwrap();
                assert_eq!(pixels[3], 0);
                std::fs::write(
                    dir.join(if dark {
                        "picker-dark-366x363.bgra"
                    } else {
                        "picker-light-366x363.bgra"
                    }),
                    pixels,
                )
                .unwrap();
            }
        }
    }
}
