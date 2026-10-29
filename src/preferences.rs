use crate::{
    core::{Core, WM_APPEARANCE, WM_REFRESH},
    render::{Painter, color, rect},
};
use anyhow::Result;
use std::sync::Arc;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::w,
};

pub const WM_PAGE_SCALE: u32 = WM_APP + 75;
const WIDTH: f32 = 396.0;
const HEIGHT: f32 = 646.0;
const PAGE_Y: f32 = 68.0;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Appearance,
    Statuses,
}
struct Preferences {
    attached: bool,
    hwnd: HWND,
    core: Arc<Core>,
    painter: Painter,
    scale: f32,
    dark: bool,
    page: Page,
    appearance: HWND,
    statuses: HWND,
    error: String,
}
fn tab_at(x: f32, y: f32) -> Option<Page> {
    if !(17.0..53.0).contains(&y) {
        return None;
    }
    if (112.0..211.0).contains(&x) {
        Some(Page::Appearance)
    } else if (211.0..310.0).contains(&x) {
        Some(Page::Statuses)
    } else {
        None
    }
}
fn fitted_scale(scale: f32, work: RECT) -> f32 {
    scale
        .min((work.bottom - work.top - 16).max(1) as f32 / HEIGHT)
        .min((work.right - work.left - 16).max(1) as f32 / WIDTH)
}
unsafe fn draw(p: &Painter, dark: bool, page: Page, error: &str) {
    unsafe {
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
            color(0.70, 0.78, 0.88, 1.0)
        } else {
            color(0.28, 0.36, 0.46, 1.0)
        };
        p.fill(rect(0.0, 0.0, WIDTH, HEIGHT), 23.0, bg);
        p.border(
            rect(0.8, 0.8, WIDTH - 1.6, HEIGHT - 1.6),
            22.2,
            color(1.0, 1.0, 1.0, if dark { 0.24 } else { 0.92 }),
            1.2,
        );
        p.text("设置", rect(25.0, 23.0, 75.0, 28.0), 2, ink);
        p.fill(
            rect(112.0, 17.0, 198.0, 36.0),
            12.0,
            color(0.25, 0.46, 0.68, if dark { 0.15 } else { 0.07 }),
        );
        for (index, (label, item)) in [("外观", Page::Appearance), ("状态", Page::Statuses)]
            .iter()
            .enumerate()
        {
            let x = 115.0 + index as f32 * 99.0;
            if *item == page {
                p.fill(
                    rect(x, 20.0, 93.0, 30.0),
                    10.0,
                    if dark {
                        color(0.25, 0.35, 0.48, 1.0)
                    } else {
                        color(1.0, 1.0, 1.0, 1.0)
                    },
                );
            }
            p.text(
                label,
                rect(x, 20.0, 93.0, 30.0),
                4,
                if *item == page { ink } else { muted },
            );
        }
        p.line(WIDTH - 39.0, 30.0, WIDTH - 31.0, 38.0, muted, 1.5);
        p.line(WIDTH - 31.0, 30.0, WIDTH - 39.0, 38.0, muted, 1.5);
        p.line(
            24.0,
            65.0,
            WIDTH - 24.0,
            65.0,
            color(muted.r, muted.g, muted.b, 0.10),
            1.0,
        );
        if !error.is_empty() {
            p.text(error, rect(24.0, 96.0, WIDTH - 48.0, 80.0), 1, muted);
        }
    }
}
impl Preferences {
    fn child(&self) -> HWND {
        match self.page {
            Page::Appearance => self.appearance,
            Page::Statuses => self.statuses,
        }
    }
    unsafe fn switch(&mut self, page: Page) -> Result<()> {
        unsafe {
            let child = match page {
                Page::Appearance => {
                    if !IsWindow(Some(self.appearance)).as_bool() {
                        self.appearance =
                            crate::settings::create_page(self.hwnd, self.core.clone(), self.scale)?;
                    }
                    self.appearance
                }
                Page::Statuses => {
                    if !IsWindow(Some(self.statuses)).as_bool() {
                        self.statuses = crate::status_manager::create_page(
                            self.hwnd,
                            self.core.clone(),
                            self.scale,
                            self.dark,
                        )?;
                    }
                    self.statuses
                }
            };
            for hwnd in [self.appearance, self.statuses] {
                if IsWindow(Some(hwnd)).as_bool() {
                    let _ = ShowWindow(
                        hwnd,
                        if hwnd == child {
                            SW_SHOWNOACTIVATE
                        } else {
                            SW_HIDE
                        },
                    );
                }
            }
            self.page = page;
            self.error.clear();
            self.layout();
            // The production window is visible. Private regression windows never take focus.
            if IsWindowVisible(self.hwnd).as_bool() {
                let _ = SetFocus(Some(child));
            }
            let _ = InvalidateRect(Some(self.hwnd), None, false);
            Ok(())
        }
    }
    unsafe fn layout(&self) {
        unsafe {
            let width = (WIDTH * self.scale).round() as i32;
            let height = (HEIGHT * self.scale).round() as i32;
            let radius = (46.0 * self.scale).round() as i32;
            let region = CreateRoundRectRgn(0, 0, width + 1, height + 1, radius, radius);
            if SetWindowRgn(self.hwnd, Some(region), true) == 0 {
                let _ = DeleteObject(region.into());
            }
            for child in [self.appearance, self.statuses] {
                if IsWindow(Some(child)).as_bool() {
                    let _ = SetWindowPos(
                        child,
                        None,
                        (8.0 * self.scale).round() as i32,
                        (PAGE_Y * self.scale).round() as i32,
                        (crate::settings::WIDTH * self.scale).round() as i32,
                        (crate::settings::HEIGHT * self.scale).round() as i32,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    );
                    let _ = SendMessageW(
                        child,
                        WM_PAGE_SCALE,
                        Some(WPARAM((self.scale * 96.0).round() as usize)),
                        Some(LPARAM(0)),
                    );
                }
            }
        }
    }
    unsafe fn forward(&mut self, msg: u32) {
        unsafe {
            if msg == WM_APPEARANCE || msg == WM_SETTINGCHANGE {
                self.dark = self.core.appearance().theme.dark(crate::ui::is_dark());
            }
            for child in [self.appearance, self.statuses] {
                if IsWindow(Some(child)).as_bool() {
                    let _ = PostMessageW(Some(child), msg, WPARAM(0), LPARAM(0));
                }
            }
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }
}
pub unsafe fn open(owner: HWND, core: Arc<Core>, scale: f32) -> Result<HWND> {
    unsafe { create(owner, core, scale, true) }
}
unsafe fn create(owner: HWND, core: Arc<Core>, scale: f32, show: bool) -> Result<HWND> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = w!("GlassTodo.Preferences");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance.into(),
            lpszClassName: class,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            ..Default::default()
        });
        let mut owner_rect = RECT::default();
        let _ = GetWindowRect(owner, &mut owner_rect);
        let mut monitor = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(
            MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST),
            &mut monitor,
        );
        let scale = fitted_scale(scale, monitor.rcWork);
        let width = (WIDTH * scale).round() as i32;
        let height = (HEIGHT * scale).round() as i32;
        let work = monitor.rcWork;
        let x = if owner_rect.right + width + 12 <= work.right {
            owner_rect.right + 12
        } else {
            owner_rect.left - width - 12
        }
        .clamp(work.left, (work.right - width).max(work.left));
        let y = owner_rect
            .top
            .clamp(work.top, (work.bottom - height).max(work.top));
        let dark = core.appearance().theme.dark(crate::ui::is_dark());
        let ptr = Box::into_raw(Box::new(Preferences {
            attached: false,
            hwnd: HWND::default(),
            core,
            painter: Painter::new()?,
            scale,
            dark,
            page: Page::Appearance,
            appearance: HWND::default(),
            statuses: HWND::default(),
            error: String::new(),
        }));
        let hwnd = match CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW,
            class,
            w!("玻璃待办 · 设置"),
            WS_POPUP | WS_CLIPCHILDREN,
            x,
            y,
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
        let result = (|| -> Result<()> {
            SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA)?;
            (*ptr).switch(Page::Appearance)?;
            Ok(())
        })();
        if let Err(error) = result {
            let _ = DestroyWindow(hwnd);
            return Err(error);
        }
        if show {
            let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some((*ptr).child()));
        }
        Ok(hwnd)
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let cs = &*(l.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            (*(cs.lpCreateParams as *mut Preferences)).hwnd = hwnd;
            return DefWindowProcW(hwnd, msg, w, l);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Preferences;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, w, l);
        }
        match msg {
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                let host = &mut *ptr;
                if host
                    .painter
                    .begin(
                        (WIDTH * host.scale).round() as i32,
                        (HEIGHT * host.scale).round() as i32,
                        host.scale,
                    )
                    .is_ok()
                {
                    draw(&host.painter, host.dark, host.page, &host.error);
                    let _ = host.painter.present(dc);
                }
                let _ = EndPaint(hwnd, &paint);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_LBUTTONUP => {
                let x = l.0 as u16 as i16 as f32 / (*ptr).scale;
                let y = (l.0 >> 16) as u16 as i16 as f32 / (*ptr).scale;
                if let Some(page) = tab_at(x, y) {
                    if let Err(error) = (*ptr).switch(page) {
                        (*ptr).error = error.to_string();
                        let _ = InvalidateRect(Some(hwnd), None, false);
                    }
                } else if rect(WIDTH - 52.0, 17.0, 34.0, 36.0).has(x, y) {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
                LRESULT(0)
            }
            WM_SETFOCUS => {
                let child = (*ptr).child();
                if IsWindow(Some(child)).as_bool() && IsWindowVisible(hwnd).as_bool() {
                    let _ = SetFocus(Some(child));
                }
                LRESULT(0)
            }
            WM_KEYDOWN => {
                if w.0 as u16 == VK_ESCAPE.0 {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                } else if w.0 as u16 == VK_TAB.0 {
                    let next = if (*ptr).page == Page::Appearance {
                        Page::Statuses
                    } else {
                        Page::Appearance
                    };
                    let _ = (*ptr).switch(next);
                }
                LRESULT(0)
            }
            WM_NCHITTEST => {
                let mut point = POINT {
                    x: l.0 as u16 as i16 as i32,
                    y: (l.0 >> 16) as u16 as i16 as i32,
                };
                let _ = ScreenToClient(hwnd, &mut point);
                let x = point.x as f32 / (*ptr).scale;
                let y = point.y as f32 / (*ptr).scale;
                if y < PAGE_Y
                    && tab_at(x, y).is_none()
                    && !rect(WIDTH - 52.0, 17.0, 34.0, 36.0).has(x, y)
                {
                    LRESULT(HTCAPTION as isize)
                } else {
                    LRESULT(HTCLIENT as isize)
                }
            }
            WM_REFRESH | WM_APPEARANCE | WM_SETTINGCHANGE => {
                (*ptr).forward(msg);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let proposed = *(l.0 as *const RECT);
                let mut monitor = MONITORINFO {
                    cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                    ..Default::default()
                };
                let _ = GetMonitorInfoW(
                    MonitorFromRect(&proposed, MONITOR_DEFAULTTONEAREST),
                    &mut monitor,
                );
                (*ptr).scale = fitted_scale(GetDpiForWindow(hwnd) as f32 / 96.0, monitor.rcWork);
                let width = (WIDTH * (*ptr).scale).round() as i32;
                let height = (HEIGHT * (*ptr).scale).round() as i32;
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    proposed.left.clamp(
                        monitor.rcWork.left,
                        (monitor.rcWork.right - width).max(monitor.rcWork.left),
                    ),
                    proposed.top.clamp(
                        monitor.rcWork.top,
                        (monitor.rcWork.bottom - height).max(monitor.rcWork.top),
                    ),
                    width,
                    height,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                (*ptr).layout();
                LRESULT(0)
            }
            WM_CLOSE => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                if (*ptr).attached {
                    let host = Box::from_raw(ptr);
                    host.core.notify();
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
    use crate::{
        appearance::{Appearance, Theme},
        statuses::StatusColor,
        store::Status,
    };
    use windows::Win32::UI::Controls::RichEdit::*;
    #[test]
    fn settings_fit_small_work_areas_and_tabs_have_separate_hits() {
        for work in [
            RECT {
                left: 0,
                top: 0,
                right: 1280,
                bottom: 680,
            },
            RECT {
                left: -1000,
                top: 0,
                right: 0,
                bottom: 560,
            },
        ] {
            let scale = fitted_scale(1.5, work);
            assert!(HEIGHT * scale <= (work.bottom - work.top - 16) as f32);
            assert!(WIDTH * scale <= (work.right - work.left - 16) as f32);
        }
        assert_eq!(tab_at(140.0, 30.0), Some(Page::Appearance));
        assert_eq!(tab_at(250.0, 30.0), Some(Page::Statuses));
        assert_eq!(tab_at(35.0, 30.0), None);
    }
    #[test]
    fn native_settings_share_one_window_and_keep_drafts_across_tabs_and_theme_changes() {
        unsafe {
            let path = std::env::temp_dir()
                .join(format!("glass-preferences-{}.sqlite3", std::process::id()));
            let core = Arc::new(Core::new(&path).unwrap());
            core.create_status("发布测试", Status::InProgress, StatusColor::Purple)
                .unwrap();
            core.create_status("发布预处理", Status::InProgress, StatusColor::Amber)
                .unwrap();
            let owner = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("private settings owner"),
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
            let hwnd = create(owner, core.clone(), 1.0, false).unwrap();
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Preferences;
            assert!(!IsWindowVisible(hwnd).as_bool());
            assert!(
                (*ptr).statuses.is_invalid(),
                "the status editor is created only on demand"
            );
            assert_eq!(GetParent((*ptr).appearance).unwrap(), hwnd);
            let mut parent_bounds = RECT::default();
            GetWindowRect(hwnd, &mut parent_bounds).unwrap();
            let mut child_bounds = RECT::default();
            GetWindowRect((*ptr).appearance, &mut child_bounds).unwrap();
            assert_eq!(
                child_bounds.left,
                parent_bounds.left + (8.0 * (*ptr).scale).round() as i32
            );
            assert_eq!(
                child_bounds.top,
                parent_bounds.top + (PAGE_Y * (*ptr).scale).round() as i32
            );
            (*ptr).switch(Page::Statuses).unwrap();
            let page = (*ptr).statuses;
            assert_eq!(GetParent(page).unwrap(), hwnd);
            assert_eq!(GetAncestor(page, GA_ROOT), hwnd);
            assert_ne!(GetWindowLongW(page, GWL_STYLE) as u32 & WS_VISIBLE.0, 0);
            assert_eq!(
                GetWindowLongW((*ptr).appearance, GWL_STYLE) as u32 & WS_VISIBLE.0,
                0
            );
            let edit = GetDlgItem(Some(page), 501).unwrap();
            SetWindowTextW(edit, w!("尚未保存的状态")).unwrap();
            (*ptr).switch(Page::Appearance).unwrap();
            (*ptr).switch(Page::Statuses).unwrap();
            assert_eq!(
                (*ptr).statuses,
                page,
                "switching tabs keeps the existing editor"
            );
            let mut text = [0u16; 32];
            let length = GetWindowTextW(edit, &mut text);
            assert_eq!(
                String::from_utf16_lossy(&text[..length as usize]),
                "尚未保存的状态"
            );
            core.preview_appearance(&Appearance {
                theme: Theme::Dark,
                ..Default::default()
            });
            core.save_appearance(&Appearance {
                theme: Theme::Dark,
                ..Default::default()
            })
            .unwrap();
            (*ptr).forward(WM_APPEARANCE);
            SendMessageW(page, WM_APPEARANCE, Some(WPARAM(0)), Some(LPARAM(0)));
            let mut format = CHARFORMAT2W::default();
            format.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
            SendMessageW(
                edit,
                EM_GETCHARFORMAT,
                Some(WPARAM(SCF_DEFAULT as usize)),
                Some(LPARAM((&mut format as *mut CHARFORMAT2W) as isize)),
            );
            assert_eq!(format.crBackColor, COLORREF(0x003f2f23));
            assert_eq!(GetWindowTextLengthW(edit), length);
            assert_eq!(
                core.statuses().unwrap().len(),
                5,
                "tab changes do not save or duplicate drafts"
            );
            DestroyWindow(hwnd).unwrap();
            assert!(!IsWindow(Some(page)).as_bool());
            DestroyWindow(owner).unwrap();
            drop(core);
            let reopened = Core::new(&path).unwrap();
            assert_eq!(reopened.statuses().unwrap().len(), 5);
            assert_eq!(reopened.appearance().theme, Theme::Dark);
            drop(reopened);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn unified_settings_render_both_pages_and_themes() {
        unsafe {
            let mut definitions: Vec<_> = [Status::Pending, Status::InProgress, Status::Done]
                .into_iter()
                .map(crate::statuses::StatusDefinition::builtin)
                .collect();
            for (id, name, color) in [
                ("custom_1", "发布测试", StatusColor::Purple),
                ("custom_2", "发布预处理", StatusColor::Amber),
            ] {
                definitions.push(crate::statuses::StatusDefinition {
                    id: id.into(),
                    name: name.into(),
                    kind: Status::InProgress,
                    color,
                    builtin: false,
                    version: 1,
                });
            }
            let dir = std::path::Path::new(".scratch/qa-preferences");
            std::fs::create_dir_all(dir).unwrap();
            let scale = 1.5;
            let width = 594usize;
            let height = 969usize;
            let mut painter = Painter::new().unwrap();
            for dark in [false, true] {
                for page in [Page::Appearance, Page::Statuses] {
                    painter.begin(width as i32, height as i32, scale).unwrap();
                    draw(&painter, dark, page, "");
                    let mut pixels = painter.test_pixels().unwrap();
                    let child = match page {
                        Page::Appearance => crate::settings::page_pixels(scale, dark),
                        Page::Statuses => {
                            crate::status_manager::page_pixels(scale, dark, &definitions)
                        }
                    };
                    for y in 0..855usize {
                        let to = ((y + 102) * width + 12) * 4;
                        pixels[to..to + 570 * 4]
                            .copy_from_slice(&child[y * 570 * 4..(y + 1) * 570 * 4]);
                    }
                    std::fs::write(
                        dir.join(format!(
                            "settings-{}-{}-594x969.bgra",
                            if dark { "dark" } else { "light" },
                            if page == Page::Appearance {
                                "appearance"
                            } else {
                                "statuses"
                            }
                        )),
                        pixels,
                    )
                    .unwrap();
                }
            }
        }
    }
}
