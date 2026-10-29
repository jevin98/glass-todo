use crate::{
    appearance::{Appearance, Theme},
    core::Core,
    render::{Painter, Rect, color, rect},
};
use anyhow::Result;
use std::sync::Arc;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Controls::WM_MOUSELEAVE, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::w,
};

const SAVE_TIMER: usize = 1;
const DESTROY: u32 = WM_APP + 31;
pub(crate) const WIDTH: f32 = 380.0;
pub(crate) const HEIGHT: f32 = 570.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Control {
    Theme(Theme),
    Opacity,
    TextSize,
    Radius,
    Reset,
    Done,
    Close,
}
const ORDER: [Control; 9] = [
    Control::Theme(Theme::System),
    Control::Theme(Theme::Light),
    Control::Theme(Theme::Dark),
    Control::Opacity,
    Control::TextSize,
    Control::Radius,
    Control::Reset,
    Control::Done,
    Control::Close,
];
impl Control {
    fn range(self) -> Option<(u8, u8)> {
        match self {
            Self::Opacity => Some((60, 100)),
            Self::TextSize => Some((12, 18)),
            Self::Radius => Some((8, 32)),
            _ => None,
        }
    }
    fn value(self, a: &Appearance) -> u8 {
        match self {
            Self::Opacity => a.opacity,
            Self::TextSize => a.text_size,
            Self::Radius => a.radius,
            _ => 0,
        }
    }
    fn set(self, a: &mut Appearance, value: u8) {
        if let Some((min, max)) = self.range() {
            let v = value.clamp(min, max);
            match self {
                Self::Opacity => a.opacity = v,
                Self::TextSize => a.text_size = v,
                Self::Radius => a.radius = v,
                _ => {}
            }
        }
    }
}
#[derive(Clone, Copy)]
struct Hit {
    r: Rect,
    control: Control,
}

struct View<'a> {
    embedded: bool,
    system_dark: bool,
    focus: Option<Control>,
    hover: Option<Control>,
    dragging: Option<Control>,
    status: &'a str,
    failed: bool,
}
// The same renderer is used by the real window and off-screen visual checks.
unsafe fn draw(p: &Painter, a: &Appearance, view: View<'_>) -> Vec<Hit> {
    unsafe {
        let View {
            embedded,
            system_dark,
            focus,
            hover,
            dragging,
            status,
            failed,
        } = view;
        let dark = a.theme.dark(system_dark);
        let bg = if dark {
            color(0.09, 0.12, 0.17, 1.0)
        } else {
            color(0.95, 0.97, 0.995, 1.0)
        };
        let ink = if dark {
            color(0.95, 0.97, 1.0, 1.0)
        } else {
            color(0.07, 0.12, 0.20, 1.0)
        };
        let muted = if dark {
            color(0.73, 0.80, 0.89, 1.0)
        } else {
            color(0.28, 0.36, 0.46, 1.0)
        };
        let accent = if dark {
            color(0.51, 0.77, 1.0, 1.0)
        } else {
            color(0.12, 0.37, 0.66, 1.0)
        };
        let quiet = if dark {
            color(1.0, 1.0, 1.0, 0.08)
        } else {
            color(0.16, 0.38, 0.63, 0.06)
        };
        if embedded {
            p.fill(rect(0.0, 0.0, WIDTH, HEIGHT), 0.0, bg);
        } else {
            p.fill(
                rect(9.0, 12.0, WIDTH - 18.0, HEIGHT - 18.0),
                24.0,
                color(0.02, 0.06, 0.12, 0.18),
            );
            p.fill(rect(8.0, 8.0, WIDTH - 16.0, HEIGHT - 16.0), 24.0, bg);
            p.border(
                rect(8.7, 8.7, WIDTH - 17.4, HEIGHT - 17.4),
                23.0,
                color(1.0, 1.0, 1.0, if dark { 0.24 } else { 0.92 }),
                1.2,
            );
        }
        p.text("外观设置", rect(25.0, 29.0, 220.0, 28.0), 2, ink);
        p.text(
            "实时预览，自动保存",
            rect(25.0, 62.0, 250.0, 22.0),
            0,
            muted,
        );
        let mut hits = Vec::new();
        let close = rect(WIDTH - 54.0, 25.0, 30.0, 30.0);
        if !embedded {
            if hover == Some(Control::Close) {
                p.fill(close, 15.0, quiet);
            }
            p.line(WIDTH - 43.0, 36.0, WIDTH - 35.0, 44.0, muted, 1.4);
            p.line(WIDTH - 35.0, 36.0, WIDTH - 43.0, 44.0, muted, 1.4);
            hits.push(Hit {
                r: close,
                control: Control::Close,
            });
        }
        p.text("主题", rect(25.0, 94.0, 100.0, 20.0), 0, muted);
        for (index, (theme, label)) in [
            (Theme::System, "跟随系统"),
            (Theme::Light, "浅色"),
            (Theme::Dark, "深色"),
        ]
        .iter()
        .enumerate()
        {
            let chip = (WIDTH - 60.0) / 3.0;
            let r = rect(24.0 + index as f32 * (chip + 6.0), 116.0, chip, 34.0);
            let control = Control::Theme(*theme);
            p.fill(
                r,
                11.0,
                if a.theme == *theme {
                    color(accent.r, accent.g, accent.b, 0.13)
                } else {
                    quiet
                },
            );
            if a.theme == *theme {
                p.border(r, 11.0, color(accent.r, accent.g, accent.b, 0.46), 1.0);
            }
            p.text(
                label,
                rect(r.x, 124.0, r.w, 23.0),
                3,
                if a.theme == *theme { accent } else { muted },
            );
            hits.push(Hit { r, control });
        }
        for (control, label, y) in [
            (Control::Opacity, "背景遮挡", 169.0),
            (Control::TextSize, "文字大小", 254.0),
            (Control::Radius, "窗口圆角", 328.0),
        ] {
            p.text(label, rect(25.0, y, 200.0, 24.0), 1, ink);
            let value = control.value(a);
            p.text(
                &format!(
                    "{}{}",
                    value,
                    if control == Control::Opacity {
                        "%"
                    } else {
                        " px"
                    }
                ),
                rect(WIDTH - 88.0, y + 2.0, 64.0, 24.0),
                3,
                accent,
            );
            if control == Control::Opacity {
                p.text(
                    "越高越清晰，文字始终保持不透明",
                    rect(25.0, 193.0, WIDTH - 50.0, 20.0),
                    0,
                    muted,
                );
            }
            let cy = if control == Control::Opacity {
                226.0
            } else {
                y + 41.0
            };
            let (min, max) = control.range().unwrap();
            let fraction = (value - min) as f32 / (max - min) as f32;
            let x = 32.0 + fraction * (WIDTH - 64.0);
            p.fill(
                rect(32.0, cy - 2.5, WIDTH - 64.0, 5.0),
                2.5,
                color(accent.r, accent.g, accent.b, 0.15),
            );
            p.fill(rect(32.0, cy - 2.5, x - 32.0, 5.0), 2.5, accent);
            if hover == Some(control) || dragging == Some(control) {
                p.circle(x, cy, 15.0, color(accent.r, accent.g, accent.b, 0.10), true);
            }
            p.circle(x, cy + 1.0, 8.5, color(0.0, 0.0, 0.0, 0.12), true);
            p.circle(
                x,
                cy,
                8.0,
                if dark {
                    color(0.85, 0.92, 1.0, 1.0)
                } else {
                    color(1.0, 1.0, 1.0, 1.0)
                },
                true,
            );
            p.circle(x, cy, 3.5, accent, true);
            hits.push(Hit {
                r: rect(24.0, cy - 17.0, WIDTH - 48.0, 34.0),
                control,
            });
        }
        p.line(
            24.0,
            398.0,
            WIDTH - 24.0,
            398.0,
            color(accent.r, accent.g, accent.b, 0.12),
            0.8,
        );
        p.text(
            status,
            rect(25.0, 410.0, WIDTH - 50.0, 26.0),
            0,
            if failed {
                color(0.80, 0.25, 0.25, 1.0)
            } else {
                muted
            },
        );
        for (control, label, r) in [
            (Control::Reset, "恢复默认", rect(24.0, 502.0, 112.0, 36.0)),
            (
                Control::Done,
                "完成",
                rect(WIDTH - 112.0, 502.0, 88.0, 36.0),
            ),
        ] {
            p.fill(
                r,
                12.0,
                if control == Control::Done {
                    accent
                } else {
                    quiet
                },
            );
            if hover == Some(control) {
                p.fill(r, 12.0, color(1.0, 1.0, 1.0, 0.09));
            }
            p.text(
                label,
                rect(r.x, r.y + 8.0, r.w, 25.0),
                3,
                if control == Control::Done {
                    color(1.0, 1.0, 1.0, 1.0)
                } else {
                    muted
                },
            );
            hits.push(Hit { r, control });
        }
        p.text(
            "Tab 切换 · 方向键调整 · Esc 关闭",
            rect(25.0, 544.0, WIDTH - 50.0, 18.0),
            0,
            muted,
        );
        if let Some(control) = focus
            && let Some(hit) = hits.iter().find(|h| h.control == control)
        {
            p.border(hit.r, 10.0, color(accent.r, accent.g, accent.b, 0.8), 1.4);
        }
        hits
    }
}

struct Settings {
    attached: bool,
    hwnd: HWND,
    container: HWND,
    core: Arc<Core>,
    appearance: Appearance,
    painter: Painter,
    scale: f32,
    hits: Vec<Hit>,
    hover: Option<Control>,
    drag: Option<Control>,
    focus: Control,
    keyboard: bool,
    dirty: bool,
    error: String,
    closing: bool,
}
impl Settings {
    unsafe fn paint(&mut self) {
        unsafe {
            if self
                .painter
                .begin(
                    (WIDTH * self.scale) as i32,
                    (HEIGHT * self.scale) as i32,
                    self.scale,
                )
                .is_ok()
            {
                let status = if !self.error.is_empty() {
                    self.error.as_str()
                } else if self.dirty {
                    "正在预览…"
                } else {
                    "已保存，重启后仍然生效"
                };
                self.hits = draw(
                    &self.painter,
                    &self.appearance,
                    View {
                        embedded: true,
                        system_dark: crate::ui::is_dark(),
                        focus: if self.keyboard {
                            Some(self.focus)
                        } else {
                            None
                        },
                        hover: self.hover,
                        dragging: self.drag,
                        status,
                        failed: !self.error.is_empty(),
                    },
                );
                let dc = GetDC(Some(self.hwnd));
                let _ = self.painter.present(dc);
                ReleaseDC(Some(self.hwnd), dc);
            }
        }
    }
    fn at(&self, x: f32, y: f32) -> Option<Control> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.r.has(x, y))
            .map(|h| h.control)
    }
    unsafe fn changed(&mut self, previous: Appearance) {
        unsafe {
            if self.appearance != previous {
                self.dirty = true;
                self.error.clear();
                self.core.preview_appearance(&self.appearance);
                SetTimer(Some(self.hwnd), SAVE_TIMER, 240, None);
            }
            self.paint();
        }
    }
    unsafe fn slider(&mut self, control: Control, x: f32) {
        unsafe {
            if let Some((min, max)) = control.range() {
                let previous = self.appearance.clone();
                control.set(
                    &mut self.appearance,
                    min + (((x - 32.0) / (WIDTH - 64.0)).clamp(0.0, 1.0) * (max - min) as f32)
                        .round() as u8,
                );
                self.changed(previous);
            }
        }
    }
    unsafe fn persist(&mut self) -> bool {
        unsafe {
            let _ = KillTimer(Some(self.hwnd), SAVE_TIMER);
            if self.dirty {
                match self.core.save_appearance(&self.appearance) {
                    Ok(()) => {
                        self.dirty = false;
                        self.error.clear();
                    }
                    Err(e) => {
                        self.error = e.to_string();
                        self.paint();
                        return false;
                    }
                }
            }
            self.paint();
            true
        }
    }
    unsafe fn activate(&mut self, control: Control) {
        unsafe {
            let previous = self.appearance.clone();
            match control {
                Control::Theme(theme) => self.appearance.theme = theme,
                Control::Reset => self.appearance = Appearance::default(),
                Control::Done | Control::Close => {
                    if self.persist() {
                        self.closing = true;
                        let _ = PostMessageW(Some(self.container), WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                    return;
                }
                _ => {}
            }
            self.changed(previous);
        }
    }
    unsafe fn key(&mut self, key: u16) {
        unsafe {
            self.keyboard = true;
            if key == VK_TAB.0 {
                let order: Vec<_> = ORDER
                    .iter()
                    .copied()
                    .filter(|control| *control != Control::Close)
                    .collect();
                let index = order.iter().position(|c| *c == self.focus).unwrap_or(0);
                self.focus = order[(index
                    + if GetKeyState(VK_SHIFT.0 as i32) < 0 {
                        order.len() - 1
                    } else {
                        1
                    })
                    % order.len()];
            } else if key == VK_ESCAPE.0 {
                self.activate(Control::Close);
                return;
            } else if key == VK_RETURN.0 || key == VK_SPACE.0 {
                self.activate(if self.focus.range().is_some() {
                    Control::Done
                } else {
                    self.focus
                });
                return;
            } else if [
                VK_LEFT.0, VK_DOWN.0, VK_RIGHT.0, VK_UP.0, VK_HOME.0, VK_END.0,
            ]
            .contains(&key)
            {
                let previous = self.appearance.clone();
                if let Some((min, max)) = self.focus.range() {
                    let old = self.focus.value(&self.appearance);
                    let value = if key == VK_HOME.0 {
                        min
                    } else if key == VK_END.0 {
                        max
                    } else if key == VK_LEFT.0 || key == VK_DOWN.0 {
                        old.saturating_sub(1)
                    } else {
                        old.saturating_add(1)
                    };
                    self.focus.set(&mut self.appearance, value);
                } else if let Control::Theme(theme) = self.focus {
                    let themes = [Theme::System, Theme::Light, Theme::Dark];
                    let index = themes.iter().position(|t| *t == theme).unwrap_or(0);
                    let next = themes[(index
                        + if key == VK_LEFT.0 || key == VK_DOWN.0 {
                            2
                        } else {
                            1
                        })
                        % 3];
                    self.focus = Control::Theme(next);
                    self.appearance.theme = next;
                }
                self.changed(previous);
                return;
            }
            self.paint();
        }
    }
}

pub unsafe fn create_page(owner: HWND, core: Arc<Core>, scale: f32) -> Result<HWND> {
    unsafe {
        let painter = Painter::new()?;
        let instance = GetModuleHandleW(None)?;
        let class = w!("GlassTodo.Appearance.Settings");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        });
        let width = (WIDTH * scale) as i32;
        let height = (HEIGHT * scale) as i32;
        let ptr = Box::into_raw(Box::new(Settings {
            attached: false,
            hwnd: HWND::default(),
            container: owner,
            appearance: core.appearance(),
            core,
            painter,
            scale,
            hits: vec![],
            hover: None,
            drag: None,
            focus: Control::Theme(Theme::System),
            keyboard: false,
            dirty: false,
            error: String::new(),
            closing: false,
        }));
        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("外观设置"),
            WS_CHILD,
            0,
            0,
            width,
            height,
            Some(owner),
            None,
            Some(instance.into()),
            Some(ptr.cast()),
        ) {
            Ok(h) => h,
            Err(e) => {
                drop(Box::from_raw(ptr));
                return Err(e.into());
            }
        };
        (*ptr).hwnd = hwnd;
        (*ptr).attached = true;
        (*ptr).paint();
        Ok(hwnd)
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let cs = &*(l.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            let ptr = cs.lpCreateParams as *mut Settings;
            (*ptr).hwnd = hwnd;
            return DefWindowProcW(hwnd, msg, w, l);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Settings;
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
            WM_LBUTTONDOWN => {
                if (*ptr).closing {
                    return LRESULT(0);
                }
                let x = l.0 as u16 as i16 as f32 / (*ptr).scale;
                let y = (l.0 >> 16) as u16 as i16 as f32 / (*ptr).scale;
                if let Some(control) = (*ptr).at(x, y) {
                    (*ptr).focus = control;
                    (*ptr).keyboard = false;
                    if control.range().is_some() {
                        (*ptr).drag = Some(control);
                        SetCapture(hwnd);
                        (*ptr).slider(control, x);
                    } else {
                        (*ptr).activate(control);
                    }
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                let x = l.0 as u16 as i16 as f32 / (*ptr).scale;
                let y = (l.0 >> 16) as u16 as i16 as f32 / (*ptr).scale;
                if let Some(control) = (*ptr).drag {
                    (*ptr).slider(control, x);
                } else {
                    let hover = (*ptr).at(x, y);
                    if hover != (*ptr).hover {
                        (*ptr).hover = hover;
                        (*ptr).paint();
                    }
                }
                let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                });
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let was_dragging = (*ptr).drag.take().is_some();
                let _ = ReleaseCapture();
                if was_dragging {
                    (*ptr).persist();
                }
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                if (*ptr).drag.take().is_some() {
                    (*ptr).persist();
                }
                LRESULT(0)
            }
            WM_MOUSELEAVE => {
                (*ptr).hover = None;
                (*ptr).paint();
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if !(*ptr).closing {
                    (*ptr).key(w.0 as u16);
                }
                LRESULT(0)
            }
            WM_KILLFOCUS => {
                (*ptr).persist();
                LRESULT(0)
            }
            WM_CLOSE => {
                (*ptr).activate(Control::Close);
                LRESULT(0)
            }
            WM_TIMER if w.0 == SAVE_TIMER => {
                (*ptr).persist();
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                (*ptr).paint();
                LRESULT(0)
            }
            WM_NCHITTEST => {
                if !(*ptr).container.is_invalid() {
                    return LRESULT(HTCLIENT as isize);
                }
                let mut p = POINT {
                    x: l.0 as u16 as i16 as i32,
                    y: (l.0 >> 16) as u16 as i16 as i32,
                };
                let _ = ScreenToClient(hwnd, &mut p);
                if p.y as f32 / (*ptr).scale < 88.0
                    && (*ptr)
                        .at(p.x as f32 / (*ptr).scale, p.y as f32 / (*ptr).scale)
                        .is_none()
                {
                    LRESULT(HTCAPTION as isize)
                } else {
                    LRESULT(HTCLIENT as isize)
                }
            }
            crate::preferences::WM_PAGE_SCALE => {
                (*ptr).scale = w.0 as f32 / 96.0;
                (*ptr).paint();
                LRESULT(0)
            }
            DESTROY => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                if !(*ptr).attached {
                    return DefWindowProcW(hwnd, msg, w, l);
                }
                let settings = Box::from_raw(ptr);
                let _ = KillTimer(Some(hwnd), SAVE_TIMER);
                if settings.dirty {
                    let _ = settings.core.save_appearance(&settings.appearance);
                }
                // An edge-docked owner may be waiting for its settings to close.
                settings.core.notify();
                DefWindowProcW(hwnd, msg, w, l)
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}

#[cfg(test)]
pub(crate) unsafe fn page_pixels(scale: f32, dark: bool) -> Vec<u8> {
    unsafe {
        let mut painter = Painter::new().unwrap();
        painter
            .begin(
                (WIDTH * scale).round() as i32,
                (HEIGHT * scale).round() as i32,
                scale,
            )
            .unwrap();
        let appearance = Appearance {
            theme: if dark { Theme::Dark } else { Theme::Light },
            ..Default::default()
        };
        draw(
            &painter,
            &appearance,
            View {
                embedded: true,
                system_dark: dark,
                focus: None,
                hover: None,
                dragging: None,
                status: "已保存，重启后仍然生效",
                failed: false,
            },
        );
        painter.test_pixels().unwrap()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_render_all_controls_and_keep_values_in_range() {
        unsafe {
            let dir = std::path::Path::new(".scratch/qa-native-render");
            std::fs::create_dir_all(dir).unwrap();
            let mut p = Painter::new().unwrap();
            for theme in [Theme::Light, Theme::Dark] {
                let mut a = Appearance {
                    theme,
                    ..Default::default()
                };
                Control::Opacity.set(&mut a, 0);
                assert_eq!(a.opacity, 60);
                Control::TextSize.set(&mut a, 255);
                assert_eq!(a.text_size, 18);
                Control::Radius.set(&mut a, 0);
                assert_eq!(a.radius, 8);
                a.opacity = 92;
                p.begin(570, 855, 1.5).unwrap();
                let hits = draw(
                    &p,
                    &a,
                    View {
                        embedded: false,
                        system_dark: false,
                        focus: None,
                        hover: None,
                        dragging: None,
                        status: "已保存，重启后仍然生效",
                        failed: false,
                    },
                );
                assert_eq!(hits.len(), 9);
                for control in ORDER {
                    assert!(hits.iter().any(|h| h.control == control));
                }
                let pixels = p.test_pixels().unwrap();
                assert_eq!(pixels[3], 0);
                std::fs::write(
                    dir.join(if theme == Theme::Light {
                        "settings-light-570x855.bgra"
                    } else {
                        "settings-dark-570x855.bgra"
                    }),
                    pixels,
                )
                .unwrap();
            }
        }
    }
}
