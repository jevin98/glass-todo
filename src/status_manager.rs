use crate::{
    core::{Core, WM_APPEARANCE, WM_REFRESH},
    render::{Painter, Rect, color, rect},
    statuses::{StatusColor, StatusDefinition},
    store::Status,
};
use anyhow::Result;
use std::sync::Arc;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::RichEdit::*, HiDpi::GetDpiForWindow, Input::KeyboardAndMouse::*, Shell::*,
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, w},
};

const WIDTH: f32 = 380.0;
const HEIGHT: f32 = 570.0;
const SAVE: u32 = WM_APP + 71;
const CHANGED: u32 = WM_APP + 72;
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
#[derive(Clone, Copy, PartialEq)]
enum Control {
    New,
    Close,
    Save,
    Delete,
    Row(usize),
    Kind(Status),
    Color(StatusColor),
}
struct Hit {
    rect: Rect,
    control: Control,
}
struct Panel {
    attached: bool,
    hwnd: HWND,
    container: HWND,
    edit: HWND,
    core: Arc<Core>,
    painter: Painter,
    definitions: Vec<StatusDefinition>,
    selected: Option<StatusDefinition>,
    kind: Status,
    color: StatusColor,
    scale: f32,
    dark: bool,
    scroll: f32,
    error: String,
    composing: bool,
    confirm_delete: bool,
}
impl Panel {
    fn editable(&self) -> bool {
        self.selected
            .as_ref()
            .is_none_or(|definition| !definition.builtin)
    }
    unsafe fn select(&mut self, definition: Option<StatusDefinition>) {
        unsafe {
            self.kind = definition
                .as_ref()
                .map(|d| d.kind)
                .unwrap_or(Status::InProgress);
            self.color = definition
                .as_ref()
                .map(|d| d.color)
                .unwrap_or(StatusColor::Blue);
            let text = wide(definition.as_ref().map(|d| d.name.as_str()).unwrap_or(""));
            self.selected = definition;
            self.error.clear();
            self.confirm_delete = false;
            let _ = SetWindowTextW(self.edit, PCWSTR(text.as_ptr()));
            let _ = EnableWindow(self.edit, self.editable());
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }
    unsafe fn value(&self) -> String {
        unsafe {
            let mut text = vec![0u16; GetWindowTextLengthW(self.edit).max(0) as usize + 1];
            let length = GetWindowTextW(self.edit, &mut text);
            String::from_utf16_lossy(&text[..length as usize])
        }
    }
    unsafe fn save(&mut self) {
        unsafe {
            if !self.editable() || self.composing {
                return;
            }
            let name = self.value();
            let creating = self.selected.is_none();
            let result = if let Some(definition) = &self.selected {
                self.core.update_status(
                    &definition.id,
                    Some(&name),
                    Some(self.kind),
                    Some(self.color),
                    Some(definition.version),
                )
            } else {
                self.core.create_status(&name, self.kind, self.color)
            };
            match result {
                Ok(definition) => {
                    if let Ok(definitions) = self.core.statuses() {
                        self.definitions = definitions;
                    }
                    if creating {
                        self.scroll = ((self.definitions.len() as f32 * 34.0) - 190.0).max(0.0);
                    }
                    self.select(Some(definition));
                }
                Err(error) => {
                    if let Ok(definitions) = self.core.statuses() {
                        self.definitions = definitions;
                    }
                    self.error = error.to_string();
                    let _ = InvalidateRect(Some(self.hwnd), None, false);
                }
            }
        }
    }
    unsafe fn delete(&mut self) {
        unsafe {
            let Some(definition) = self.selected.as_ref().filter(|d| !d.builtin).cloned() else {
                return;
            };
            if !self.confirm_delete {
                self.confirm_delete = true;
                self.error = "再次点删除：相关待办将移至「未完成」".into();
                let _ = InvalidateRect(Some(self.hwnd), None, false);
                return;
            }
            match self
                .core
                .delete_status(&definition.id, "pending", Some(definition.version))
            {
                Ok(()) => {
                    if let Ok(definitions) = self.core.statuses() {
                        self.definitions = definitions;
                    }
                    self.scroll = 0.0;
                    self.select(None);
                }
                Err(error) => {
                    if let Ok(definitions) = self.core.statuses() {
                        self.definitions = definitions;
                    }
                    self.error = error.to_string();
                    let _ = InvalidateRect(Some(self.hwnd), None, false);
                }
            }
        }
    }
    unsafe fn draw(&self) {
        unsafe {
            draw(
                &self.painter,
                &self.definitions,
                self.selected.as_ref(),
                self.kind,
                self.color,
                self.dark,
                self.scroll,
                &self.error,
                !self.container.is_invalid(),
            )
        }
    }
    unsafe fn click(&mut self, x: f32, y: f32) {
        unsafe {
            let control = self
                .draw_hits()
                .into_iter()
                .find(|hit| hit.rect.has(x, y))
                .map(|hit| hit.control);
            match control {
                Some(Control::New) => {
                    self.select(None);
                    let _ = SetFocus(Some(self.edit));
                }
                Some(Control::Close) => {
                    let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                Some(Control::Save) => self.save(),
                Some(Control::Delete) => self.delete(),
                Some(Control::Row(index)) => self.select(self.definitions.get(index).cloned()),
                Some(Control::Kind(kind)) => {
                    self.kind = kind;
                    self.confirm_delete = false;
                    self.error.clear();
                }
                Some(Control::Color(color)) => {
                    self.color = color;
                    self.confirm_delete = false;
                    self.error.clear();
                }
                None => {}
            }
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }
    fn draw_hits(&self) -> Vec<Hit> {
        let editable = self.editable();
        let mut hits = vec![
            Hit {
                rect: rect(280.0, 18.0, 80.0, 32.0),
                control: Control::New,
            },
            Hit {
                rect: rect(252.0, 522.0, 108.0, 32.0),
                control: Control::Close,
            },
        ];
        for (index, _) in self.definitions.iter().enumerate() {
            let y = 70.0 + index as f32 * 34.0 - self.scroll;
            if y >= 69.0 && y + 32.0 <= 260.0 {
                hits.push(Hit {
                    rect: rect(20.0, y, 340.0, 32.0),
                    control: Control::Row(index),
                });
            }
        }
        if editable {
            hits.push(Hit {
                rect: rect(120.0, 522.0, 120.0, 32.0),
                control: Control::Save,
            });
            if self.selected.is_some() {
                hits.push(Hit {
                    rect: rect(20.0, 522.0, 88.0, 32.0),
                    control: Control::Delete,
                });
            }
            for (index, kind) in [Status::Pending, Status::InProgress, Status::Done]
                .iter()
                .enumerate()
            {
                hits.push(Hit {
                    rect: rect(20.0 + index as f32 * 116.0, 370.0, 108.0, 30.0),
                    control: Control::Kind(*kind),
                });
            }
            for (index, color) in StatusColor::ALL.iter().enumerate() {
                hits.push(Hit {
                    rect: rect(24.0 + index as f32 * 56.0, 424.0, 40.0, 36.0),
                    control: Control::Color(*color),
                });
            }
        }
        hits
    }
}

#[allow(clippy::too_many_arguments)]
unsafe fn draw(
    p: &Painter,
    definitions: &[StatusDefinition],
    selected: Option<&StatusDefinition>,
    kind: Status,
    shade: StatusColor,
    dark: bool,
    scroll: f32,
    error: &str,
    embedded: bool,
) {
    unsafe {
        let background = if dark {
            color(0.105, 0.14, 0.19, 1.0)
        } else {
            color(0.945, 0.965, 0.988, 1.0)
        };
        let ink = if dark {
            color(0.94, 0.96, 1.0, 1.0)
        } else {
            color(0.08, 0.14, 0.22, 1.0)
        };
        let muted = if dark {
            color(0.70, 0.77, 0.85, 1.0)
        } else {
            color(0.35, 0.43, 0.53, 1.0)
        };
        let accent = if dark {
            color(0.55, 0.78, 1.0, 1.0)
        } else {
            color(0.14, 0.40, 0.68, 1.0)
        };
        let editable = selected.is_none_or(|d| !d.builtin);
        p.fill(
            rect(0.0, 0.0, WIDTH, HEIGHT),
            if embedded { 0.0 } else { 23.0 },
            background,
        );
        if !embedded {
            p.border(
                rect(0.8, 0.8, WIDTH - 1.6, HEIGHT - 1.6),
                22.0,
                color(1.0, 1.0, 1.0, if dark { 0.18 } else { 0.92 }),
                1.3,
            );
        }
        p.text("状态管理", rect(20.0, 19.0, 200.0, 30.0), 2, ink);
        p.fill(
            rect(280.0, 18.0, 80.0, 32.0),
            12.0,
            color(accent.r, accent.g, accent.b, 0.12),
        );
        p.text("＋ 新增", rect(280.0, 18.0, 80.0, 32.0), 4, accent);
        p.text(
            "点击待办上的状态标签，即可切换",
            rect(20.0, 48.0, 340.0, 20.0),
            0,
            muted,
        );
        p.clip(rect(20.0, 69.0, 340.0, 191.0));
        for (index, definition) in definitions.iter().enumerate() {
            let y = 70.0 + index as f32 * 34.0 - scroll;
            if y + 32.0 < 69.0 || y > 260.0 {
                continue;
            }
            let active = selected.is_some_and(|value| value.id == definition.id);
            if active {
                p.fill(
                    rect(20.0, y, 340.0, 32.0),
                    9.0,
                    color(accent.r, accent.g, accent.b, 0.10),
                );
            }
            let (r, g, b) = definition.color.rgb(dark);
            p.circle(36.0, y + 16.0, 4.0, color(r, g, b, 1.0), true);
            p.text(&definition.name, rect(49.0, y + 5.0, 180.0, 25.0), 1, ink);
            let label = if definition.builtin {
                "内置"
            } else {
                match definition.kind {
                    Status::Pending => "未完成",
                    Status::InProgress => "进行中",
                    Status::Done => "已完成",
                }
            };
            p.text(label, rect(274.0, y, 70.0, 32.0), 4, muted);
        }
        p.unclip();
        let total = definitions.len() as f32 * 34.0;
        if total > 190.0 {
            let thumb = (190.0 * 190.0 / total).max(24.0);
            let y = 70.0 + scroll / (total - 190.0) * (190.0 - thumb);
            p.fill(
                rect(356.0, y, 3.0, thumb),
                1.5,
                color(muted.r, muted.g, muted.b, 0.28),
            );
        }
        p.text(
            if selected.is_none() {
                "新状态名称"
            } else {
                "状态名称"
            },
            rect(20.0, 275.0, 220.0, 23.0),
            0,
            muted,
        );
        let field = if dark {
            color(35.0 / 255.0, 47.0 / 255.0, 63.0 / 255.0, 1.0)
        } else {
            color(230.0 / 255.0, 237.0 / 255.0, 245.0 / 255.0, 1.0)
        };
        p.fill(rect(20.0, 301.0, 340.0, 40.0), 12.0, field);
        p.border(
            rect(20.0, 301.0, 340.0, 40.0),
            12.0,
            color(accent.r, accent.g, accent.b, 0.2),
            1.0,
        );
        p.text("归类", rect(20.0, 346.0, 200.0, 22.0), 0, muted);
        for (index, value) in [Status::Pending, Status::InProgress, Status::Done]
            .iter()
            .enumerate()
        {
            let r = rect(20.0 + index as f32 * 116.0, 370.0, 108.0, 30.0);
            p.fill(
                r,
                10.0,
                color(
                    accent.r,
                    accent.g,
                    accent.b,
                    if *value == kind { 0.16 } else { 0.05 },
                ),
            );
            p.text(
                &StatusDefinition::builtin(*value).name,
                r,
                4,
                if *value == kind { accent } else { muted },
            );
        }
        p.text("标签颜色", rect(20.0, 404.0, 200.0, 21.0), 0, muted);
        for (index, value) in StatusColor::ALL.iter().enumerate() {
            let (r, g, b) = value.rgb(dark);
            let x = 44.0 + index as f32 * 56.0;
            p.circle(
                x,
                442.0,
                10.0,
                color(r, g, b, if editable { 1.0 } else { 0.4 }),
                true,
            );
            if *value == shade {
                p.circle(x, 442.0, 14.0, accent, false);
            }
        }
        p.text(
            if !error.is_empty() {
                error
            } else if editable {
                "名称 1–10 个字符，修改后点击保存"
            } else {
                "内置状态固定，可点击新增自定义状态"
            },
            rect(20.0, 475.0, 340.0, 38.0),
            0,
            if error.is_empty() {
                muted
            } else {
                color(0.78, 0.18, 0.24, 1.0)
            },
        );
        for (r, label, enabled, primary) in [
            (
                rect(20.0, 522.0, 88.0, 32.0),
                "删除",
                editable && selected.is_some(),
                false,
            ),
            (rect(120.0, 522.0, 120.0, 32.0), "保存状态", editable, true),
            (rect(252.0, 522.0, 108.0, 32.0), "关闭", true, false),
        ] {
            p.fill(
                r,
                11.0,
                if primary && enabled {
                    accent
                } else {
                    color(
                        accent.r,
                        accent.g,
                        accent.b,
                        if enabled { 0.10 } else { 0.04 },
                    )
                },
            );
            p.text(
                label,
                r,
                4,
                if !enabled {
                    color(muted.r, muted.g, muted.b, 0.45)
                } else if primary {
                    if dark {
                        color(0.07, 0.12, 0.18, 1.0)
                    } else {
                        color(1.0, 1.0, 1.0, 1.0)
                    }
                } else {
                    ink
                },
            );
        }
    }
}

pub unsafe fn create_page(owner: HWND, core: Arc<Core>, scale: f32, dark: bool) -> Result<HWND> {
    unsafe { create_impl(owner, core, scale, dark, false, true) }
}
#[cfg(test)]
unsafe fn create(owner: HWND, core: Arc<Core>, scale: f32, dark: bool, show: bool) -> Result<HWND> {
    unsafe { create_impl(owner, core, scale, dark, show, false) }
}
unsafe fn create_impl(
    owner: HWND,
    core: Arc<Core>,
    scale: f32,
    dark: bool,
    show: bool,
    embedded: bool,
) -> Result<HWND> {
    unsafe {
        crate::ui::load_native_input()?;
        let instance = GetModuleHandleW(None)?;
        let class = w!("GlassTodo.StatusManager");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        });
        let definitions = core.statuses()?;
        let mut owner_rect = RECT::default();
        GetWindowRect(owner, &mut owner_rect)?;
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(
            MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        );
        let (width, height) = (
            (WIDTH * scale).round() as i32,
            (HEIGHT * scale).round() as i32,
        );
        let x = owner_rect.left.clamp(
            monitor.rcWork.left,
            (monitor.rcWork.right - width).max(monitor.rcWork.left),
        );
        let y = owner_rect.top.clamp(
            monitor.rcWork.top,
            (monitor.rcWork.bottom - height).max(monitor.rcWork.top),
        );
        let ptr = Box::into_raw(Box::new(Panel {
            attached: false,
            hwnd: HWND::default(),
            container: if embedded { owner } else { HWND::default() },
            edit: HWND::default(),
            core,
            painter: Painter::new()?,
            definitions,
            selected: None,
            kind: Status::InProgress,
            color: StatusColor::Blue,
            scale,
            dark,
            scroll: 0.0,
            error: String::new(),
            composing: false,
            confirm_delete: false,
        }));
        let hwnd = match CreateWindowExW(
            if embedded {
                WINDOW_EX_STYLE(0)
            } else {
                WS_EX_LAYERED | WS_EX_TOOLWINDOW
            },
            class,
            w!("状态管理"),
            (if embedded { WS_CHILD } else { WS_POPUP }) | WS_CLIPCHILDREN,
            if embedded { 0 } else { x },
            if embedded { 0 } else { y },
            width,
            height,
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
        let initialized = (|| -> Result<()> {
            if !embedded {
                SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
                let region = CreateRoundRectRgn(
                    0,
                    0,
                    width + 1,
                    height + 1,
                    (46.0 * scale) as i32,
                    (46.0 * scale) as i32,
                );
                if SetWindowRgn(hwnd, Some(region), false) == 0 {
                    let _ = DeleteObject(HGDIOBJ(region.0));
                }
            }
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("RICHEDIT50W"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                (30.0 * scale) as i32,
                (310.0 * scale) as i32,
                (320.0 * scale) as i32,
                (24.0 * scale) as i32,
                Some(hwnd),
                Some(HMENU(501usize as *mut _)),
                Some(instance.into()),
                None,
            )?;
            (*ptr).edit = edit;
            crate::ui::configure_native_input(edit, 14, dark)?;
            SendMessageW(edit, EM_EXLIMITTEXT, Some(WPARAM(0)), Some(LPARAM(20)));
            anyhow::ensure!(
                SetWindowSubclass(edit, Some(edit_proc), 1, ptr as usize).as_bool(),
                "初始化状态名称输入失败"
            );
            Ok(())
        })();
        if let Err(error) = initialized {
            let _ = DestroyWindow(hwnd);
            return Err(error);
        }
        if show {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some((*ptr).edit));
        }
        Ok(hwnd)
    }
}

unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let panel = data as *mut Panel;
        if msg == WM_IME_STARTCOMPOSITION {
            (*panel).composing = true;
        }
        if msg == WM_IME_ENDCOMPOSITION {
            (*panel).composing = false;
            let _ = PostMessageW(Some((*panel).hwnd), WM_APPEARANCE, WPARAM(0), LPARAM(0));
        }
        if msg == WM_KEYDOWN
            && !(*panel).composing
            && (w.0 as u16 == VK_RETURN.0 || w.0 as u16 == VK_ESCAPE.0)
        {
            let _ = PostMessageW(
                Some((*panel).hwnd),
                if w.0 as u16 == VK_RETURN.0 {
                    SAVE
                } else {
                    WM_CLOSE
                },
                WPARAM(0),
                LPARAM(0),
            );
            return LRESULT(0);
        }
        if msg == WM_CHAR && !(*panel).composing && matches!(w.0, 13 | 27) {
            return LRESULT(0);
        }
        DefSubclassProc(hwnd, msg, w, l)
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let cs = &*(l.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            (*(cs.lpCreateParams as *mut Panel)).hwnd = hwnd;
            return DefWindowProcW(hwnd, msg, w, l);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Panel;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, w, l);
        }
        match msg {
            crate::preferences::WM_PAGE_SCALE => {
                (*ptr).scale = w.0 as f32 / 96.0;
                let scale = (*ptr).scale;
                let _ = SendMessageW(
                    (*ptr).edit,
                    EM_SETZOOM,
                    Some(WPARAM((scale * 9600.0).round() as usize)),
                    Some(LPARAM(GetDpiForWindow((*ptr).edit) as isize * 100)),
                );
                let _ = SetWindowPos(
                    (*ptr).edit,
                    None,
                    (30.0 * scale) as i32,
                    (310.0 * scale) as i32,
                    (320.0 * scale) as i32,
                    (24.0 * scale) as i32,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_SETFOCUS => {
                if IsWindowVisible(hwnd).as_bool() && IsWindowEnabled((*ptr).edit).as_bool() {
                    let _ = SetFocus(Some((*ptr).edit));
                }
                LRESULT(0)
            }
            WM_APPEARANCE | WM_SETTINGCHANGE => {
                let dark = (*ptr).core.appearance().theme.dark(crate::ui::is_dark());
                if !(*ptr).composing {
                    (*ptr).dark = dark;
                    let _ = crate::ui::apply_native_input_style((*ptr).edit, 14, dark);
                    let _ = InvalidateRect(Some(hwnd), None, false);
                }
                LRESULT(0)
            }
            WM_REFRESH => {
                if let Ok(definitions) = (*ptr).core.statuses() {
                    (*ptr).definitions = definitions;
                }
                (*ptr).scroll = (*ptr)
                    .scroll
                    .min(((*ptr).definitions.len() as f32 * 34.0 - 190.0).max(0.0));
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                let panel = &mut *ptr;
                if panel
                    .painter
                    .begin(
                        (WIDTH * panel.scale).round() as i32,
                        (HEIGHT * panel.scale).round() as i32,
                        panel.scale,
                    )
                    .is_ok()
                {
                    panel.draw();
                    let _ = panel.painter.present(dc);
                }
                let _ = EndPaint(hwnd, &paint);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_LBUTTONDOWN => {
                (*ptr).click(
                    l.0 as u16 as i16 as f32 / (*ptr).scale,
                    (l.0 >> 16) as u16 as i16 as f32 / (*ptr).scale,
                );
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = (w.0 >> 16) as u16 as i16 as f32;
                (*ptr).scroll = ((*ptr).scroll - delta / 120.0 * 68.0).clamp(
                    0.0,
                    ((*ptr).definitions.len() as f32 * 34.0 - 190.0).max(0.0),
                );
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            WM_COMMAND => {
                if (w.0 >> 16) as u32 == EN_CHANGE {
                    let _ = PostMessageW(Some(hwnd), CHANGED, WPARAM(0), LPARAM(0));
                }
                LRESULT(0)
            }
            CHANGED => {
                (*ptr).error.clear();
                (*ptr).confirm_delete = false;
                let _ = InvalidateRect(Some(hwnd), None, false);
                LRESULT(0)
            }
            SAVE => {
                (*ptr).save();
                LRESULT(0)
            }
            WM_KEYDOWN if w.0 as u16 == VK_ESCAPE.0 => {
                let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                LRESULT(0)
            }
            WM_NCHITTEST => {
                if !(*ptr).container.is_invalid() {
                    return LRESULT(HTCLIENT as isize);
                }
                let mut point = POINT {
                    x: l.0 as u16 as i16 as i32,
                    y: (l.0 >> 16) as u16 as i16 as i32,
                };
                let _ = ScreenToClient(hwnd, &mut point);
                if point.y as f32 / (*ptr).scale < 48.0 && point.x as f32 / (*ptr).scale < 260.0 {
                    LRESULT(HTCAPTION as isize)
                } else {
                    LRESULT(HTCLIENT as isize)
                }
            }
            WM_CLOSE => {
                if (*ptr).container.is_invalid() {
                    let _ = DestroyWindow(hwnd);
                } else {
                    let _ = PostMessageW(Some((*ptr).container), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                if (*ptr).attached {
                    let panel = Box::from_raw(ptr);
                    panel.core.notify();
                }
                DefWindowProcW(hwnd, msg, w, l)
            }
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}

#[cfg(test)]
pub(crate) unsafe fn page_pixels(
    scale: f32,
    dark: bool,
    definitions: &[StatusDefinition],
) -> Vec<u8> {
    unsafe {
        let mut painter = Painter::new().unwrap();
        painter
            .begin(
                (WIDTH * scale).round() as i32,
                (HEIGHT * scale).round() as i32,
                scale,
            )
            .unwrap();
        draw(
            &painter,
            definitions,
            definitions.get(3),
            Status::InProgress,
            StatusColor::Purple,
            dark,
            0.0,
            "",
            true,
        );
        painter.test_pixels().unwrap()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_status_form_saves_edits_and_reassigns_without_deleting_todos() {
        unsafe {
            let path = std::env::temp_dir()
                .join(format!("glass-status-form-{}.sqlite3", std::process::id()));
            let core = Arc::new(Core::new(&path).unwrap());
            let owner = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("private owner"),
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
            let hwnd = create(owner, core.clone(), 1.0, false, false).unwrap();
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Panel;
            assert!(!IsWindowVisible(hwnd).as_bool());
            assert_eq!(
                (*ptr)
                    .draw_hits()
                    .iter()
                    .filter(|hit| matches!(hit.control, Control::Color(_)))
                    .count(),
                6
            );
            assert_eq!(
                (*ptr)
                    .draw_hits()
                    .iter()
                    .filter(|hit| matches!(hit.control, Control::Kind(_)))
                    .count(),
                3
            );
            SetWindowTextW((*ptr).edit, w!("待审核")).unwrap();
            (*ptr).kind = Status::InProgress;
            (*ptr).color = StatusColor::Purple;
            (*ptr).save();
            let definition = (*ptr).selected.clone().unwrap();
            assert_eq!(
                (definition.name.as_str(), definition.color),
                ("待审核", StatusColor::Purple)
            );
            let todo = core.create("必须完整保留的正文").unwrap();
            core.update_with_status(
                todo.id,
                None,
                None,
                Some(&definition.id),
                Some(todo.version),
            )
            .unwrap();
            SetWindowTextW((*ptr).edit, w!("待验收")).unwrap();
            (*ptr).kind = Status::Done;
            (*ptr).save();
            assert_eq!(
                (
                    core.get(todo.id).unwrap().status_name.as_str(),
                    core.get(todo.id).unwrap().status
                ),
                ("待验收", Status::Done)
            );
            (*ptr).delete();
            assert!(
                core.statuses()
                    .unwrap()
                    .iter()
                    .any(|d| d.id == definition.id),
                "first delete click only asks for confirmation"
            );
            (*ptr).delete();
            assert!(
                !core
                    .statuses()
                    .unwrap()
                    .iter()
                    .any(|d| d.id == definition.id)
            );
            let restored = core.get(todo.id).unwrap();
            assert_eq!(
                (restored.content.as_str(), restored.status_id.as_str()),
                ("必须完整保留的正文", "pending")
            );
            (*ptr).select(Some(StatusDefinition::builtin(Status::Done)));
            assert!(!IsWindowEnabled((*ptr).edit).as_bool());
            assert!(!(*ptr).draw_hits().iter().any(|hit| matches!(
                hit.control,
                Control::Save | Control::Delete | Control::Kind(_)
            )));
            assert!(!IsWindowVisible(hwnd).as_bool());
            DestroyWindow(hwnd).unwrap();
            DestroyWindow(owner).unwrap();
            drop(core);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn status_manager_render_supports_both_themes_and_full_form_controls() {
        unsafe {
            let directory = std::path::PathBuf::from(".scratch/qa-status-manager");
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join(format!("{}.sqlite3", std::process::id()));
            let core = Arc::new(Core::new(&path).unwrap());
            core.create_status("等待反馈", Status::InProgress, StatusColor::Amber)
                .unwrap();
            let definitions = core.statuses().unwrap();
            let mut painter = Painter::new().unwrap();
            for dark in [false, true] {
                painter.begin(570, 855, 1.5).unwrap();
                draw(
                    &painter,
                    &definitions,
                    None,
                    Status::InProgress,
                    StatusColor::Blue,
                    dark,
                    0.0,
                    "",
                    false,
                );
                let pixels = painter.test_pixels().unwrap();
                assert_eq!(pixels[3], 0);
                assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[3] == 255));
                std::fs::write(
                    directory.join(if dark {
                        "manager-dark-570x855.bgra"
                    } else {
                        "manager-light-570x855.bgra"
                    }),
                    pixels,
                )
                .unwrap();
            }
        }
    }
}
