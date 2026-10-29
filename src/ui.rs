use crate::{
    appearance::Appearance,
    config::Config,
    core::{Core, WM_APPEARANCE, WM_REFRESH},
    docking::{self, Dock, Layout},
    motion::Spring,
    render::{Painter, Rect, color, rect},
    store::{DEFAULT_HEIGHT, DEFAULT_WIDTH, Status, Todo, WindowState, split_todo_blocks},
};
use anyhow::Result;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, atomic::Ordering},
    time::Instant,
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        System::{
            LibraryLoader::{GetModuleHandleW, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
            Registry::*,
        },
        UI::{
            Controls::{RichEdit::*, *},
            HiDpi::*,
            Input::{Ime::*, KeyboardAndMouse::*},
            Shell::*,
            WindowsAndMessaging::*,
        },
    },
    core::{PCWSTR, w},
};

const WM_TRAY: u32 = WM_APP + 9;
const WM_SAVE_EDIT: u32 = WM_APP + 10;
const SAVE_TIMER: usize = 11;
const EXIT_TIMER: usize = 12;
const MOTION_TIMER: usize = 13;
const EDIT_ID: usize = 100;
const DOCK_HIDE_TIMER: usize = 14;
const RESIZE_CORNER: f32 = 44.0;
const RESIZE_EDGE: f32 = 22.0;

fn track_limits(scale: f32, collapsed: bool, folding: bool, work: RECT) -> (POINT, POINT) {
    let compact = collapsed || folding;
    let minimum = POINT {
        x: (if compact { 288.0 } else { DEFAULT_WIDTH as f32 } * scale).round() as i32,
        y: (if compact { 86.0 } else { DEFAULT_HEIGHT as f32 } * scale).round() as i32,
    };
    let maximum = if collapsed && !folding {
        minimum
    } else {
        POINT {
            x: (work.right - work.left).max(minimum.x),
            y: (work.bottom - work.top).max(minimum.y),
        }
    };
    (minimum, maximum)
}

unsafe fn window_work_area(hwnd: HWND) -> RECT {
    unsafe { monitor_work_area(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)) }
}
unsafe fn monitor_work_area(monitor: HMONITOR) -> RECT {
    unsafe {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).as_bool() {
            info.rcWork
        } else {
            work_area()
        }
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn class_name(port: u16) -> Vec<u16> {
    wide(&format!("GlassTodo.Native.{port}"))
}
fn xparam(l: LPARAM) -> i32 {
    l.0 as u16 as i16 as i32
}
fn yparam(l: LPARAM) -> i32 {
    (l.0 >> 16) as u16 as i16 as i32
}
fn cursor_position() -> Option<POINT> {
    unsafe {
        let mut point = POINT::default();
        GetCursorPos(&mut point).ok().map(|_| point)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Settings,
    Pin,
    Fold,
    Add,
    Completed,
    Check(i64),
    Progress(i64),
    Delete(i64),
    UndoDelete,
    Row(i64),
}
#[derive(Clone, Copy)]
struct Hit {
    r: Rect,
    action: Action,
}
fn drag_destination(
    ids: &[i64],
    rows: &[(i64, f32)],
    dragged: i64,
    y: f32,
    fallback: usize,
) -> usize {
    let mut rows: Vec<_> = rows
        .iter()
        .filter(|(id, _)| *id != dragged && ids.contains(id))
        .copied()
        .collect();
    rows.sort_by(|a, b| a.1.total_cmp(&b.1));
    rows.iter()
        .rfind(|(_, mid)| *mid < y)
        .and_then(|(id, _)| ids.iter().position(|v| v == id))
        .map(|i| i + 1)
        .or_else(|| {
            rows.first()
                .and_then(|(id, _)| ids.iter().position(|v| v == id))
        })
        .unwrap_or(fallback.min(ids.len()))
}
struct AnimatedRow {
    todo: Todo,
    y: Spring,
    height: Spring,
    alpha: Spring,
    check: Spring,
    progress: Spring,
    seen: bool,
    retiring: bool,
}
struct Motion {
    enabled: bool,
    timer: bool,
    last: Instant,
    fold: Spring,
    completed: Spring,
    scroll: Spring,
    hover: Spring,
    pulse: Spring,
    hovered: Option<Action>,
    old_hover: Option<Action>,
    pressed: Option<Action>,
    fold_size: Option<(f32, f32)>,
    rows: HashMap<i64, AnimatedRow>,
    initialized: bool,
}
impl Motion {
    fn new(state: &WindowState, enabled: bool) -> Self {
        Self {
            enabled,
            timer: false,
            last: Instant::now(),
            fold: Spring::new(if state.collapsed { 1.0 } else { 0.0 }),
            completed: Spring::new(if state.completed_open { 1.0 } else { 0.0 }),
            scroll: Spring::new(0.0),
            hover: Spring::new(1.0),
            pulse: Spring::new(0.0),
            hovered: None,
            old_hover: None,
            pressed: None,
            fold_size: None,
            rows: HashMap::new(),
            initialized: false,
        }
    }
    fn hover(&self, action: Action) -> f32 {
        if self.hovered == Some(action) {
            self.hover.value
        } else if self.old_hover == Some(action) {
            1.0 - self.hover.value
        } else {
            0.0
        }
    }
    fn feedback(&self, action: Action) -> f32 {
        self.hover(action) * 0.10
            + if self.pressed == Some(action) {
                self.pulse.value * 0.18
            } else {
                0.0
            }
    }
    fn place(&mut self, todo: &Todo, y: f32, height: f32) {
        let animate = self.enabled && self.initialized;
        let row = self.rows.entry(todo.id).or_insert_with(|| AnimatedRow {
            todo: todo.clone(),
            y: Spring::new(y + if animate { 12.0 } else { 0.0 }),
            height: Spring::new(height),
            alpha: Spring::new(if animate { 0.0 } else { 1.0 }),
            check: Spring::new(if todo.status == Status::Done {
                1.0
            } else {
                0.0
            }),
            progress: Spring::new(if todo.status == Status::InProgress {
                1.0
            } else {
                0.0
            }),
            seen: true,
            retiring: false,
        });
        row.todo = todo.clone();
        row.seen = true;
        row.retiring = false;
        row.y.set(y);
        row.height.set(height);
        row.alpha.set(1.0);
        row.check.set(if todo.status == Status::Done {
            1.0
        } else {
            0.0
        });
        row.progress.set(if todo.status == Status::InProgress {
            1.0
        } else {
            0.0
        });
    }
    fn step(&mut self, seconds: f32) {
        let dt = if self.enabled { seconds } else { 5.0 };
        for s in [
            &mut self.fold,
            &mut self.completed,
            &mut self.scroll,
            &mut self.hover,
            &mut self.pulse,
        ] {
            s.step(dt);
        }
        for row in self.rows.values_mut() {
            for s in [
                &mut row.y,
                &mut row.height,
                &mut row.alpha,
                &mut row.check,
                &mut row.progress,
            ] {
                s.step(dt);
            }
        }
        self.rows.retain(|_, r| !r.retiring || r.alpha.value > 0.0);
    }
    fn active(&self) -> bool {
        [
            self.fold,
            self.completed,
            self.scroll,
            self.hover,
            self.pulse,
        ]
        .iter()
        .any(Spring::active)
            || self.rows.values().any(|r| {
                [r.y, r.height, r.alpha, r.check, r.progress]
                    .iter()
                    .any(Spring::active)
            })
    }
}
pub(crate) unsafe fn animations_enabled() -> bool {
    unsafe {
        let mut enabled = 1i32;
        let _ = SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some((&mut enabled as *mut i32).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        enabled != 0
    }
}
struct Ui {
    hwnd: HWND,
    core: Arc<Core>,
    cfg: Config,
    state: WindowState,
    appearance: Appearance,
    settings: HWND,
    status_picker: HWND,
    painter: Painter,
    todos: Vec<Todo>,
    hits: Vec<Hit>,
    scale: f32,
    scroll: f32,
    selected: Option<i64>,
    pending_delete: Option<(i64, i64)>,
    can_undo_delete: bool,
    reveal_todo: Option<i64>,
    pressed: Option<(i64, POINT)>,
    dragging: bool,
    drag_y: f32,
    editor: HWND,
    backdrop: HWND,
    dark: bool,
    done_scroll: f32,
    completed_area: Rect,
    dispatching: bool,
    deferred_refresh: bool,
    motion: Motion,
    dock: Option<Dock>,
    dock_hide_due: Option<Instant>,
    native_drag: bool,
    moving_window: bool,
}

pub fn activate_existing(port: u16) -> bool {
    unsafe {
        let class = class_name(port);
        if let Ok(hwnd) = FindWindowW(PCWSTR(class.as_ptr()), None) {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            let _ = PostMessageW(Some(hwnd), WM_REFRESH, WPARAM(0), LPARAM(0));
            return true;
        }
        false
    }
}
/// Graceful application lifecycle command, also compatible with 0.1.0/0.1.1.
pub fn request_exit(port: u16) -> Result<()> {
    unsafe {
        let class = class_name(port);
        if let Ok(hwnd) = FindWindowW(PCWSTR(class.as_ptr()), None) {
            PostMessageW(Some(hwnd), WM_TIMER, WPARAM(EXIT_TIMER), LPARAM(0))?;
        }
        Ok(())
    }
}

pub fn run(core: Arc<Core>, cfg: Config) -> Result<()> {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(None)?;
        let class = class_name(cfg.port);
        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hIcon: LoadIconW(
                Some(instance.into()),
                PCWSTR(std::ptr::without_provenance(1)),
            )
            .unwrap_or_default(),
            hInstance: instance.into(),
            lpszClassName: PCWSTR(class.as_ptr()),
            lpfnWndProc: Some(window_proc),
            ..Default::default()
        };
        anyhow::ensure!(RegisterClassW(&wc) != 0, "注册窗口失败");
        let mut state = core.window().normalized_size();
        if state.dock.is_some() {
            state.collapsed = false;
        }
        if cfg.no_autostart {
            state.autostart = false;
        }
        if state.autostart
            && let Err(e) = set_autostart(true, &cfg)
        {
            cfg.log(&format!("登录自启设置失败: {e:#}"));
        }
        let appearance = core.appearance();
        let mut painter = Painter::new()?;
        painter.set_text_size(appearance.text_size)?;
        let motion = Motion::new(&state, animations_enabled());
        let mut app = Box::new(Ui {
            hwnd: HWND::default(),
            core: core.clone(),
            cfg: cfg.clone(),
            state,
            painter,
            todos: core.list()?,
            hits: vec![],
            scale: 1.0,
            scroll: 0.0,
            selected: None,
            pending_delete: None,
            can_undo_delete: core.can_undo_delete()?,
            reveal_todo: None,
            pressed: None,
            dragging: false,
            drag_y: 0.0,
            editor: HWND::default(),
            backdrop: HWND::default(),
            dark: appearance.theme.dark(is_dark()),
            appearance,
            settings: HWND::default(),
            status_picker: HWND::default(),
            done_scroll: 0.0,
            completed_area: rect(0.0, 0.0, 0.0, 0.0),
            dispatching: false,
            deferred_refresh: false,
            motion,
            dock: None,
            dock_hide_due: None,
            native_drag: false,
            moving_window: false,
        });
        let pointer = &mut *app as *mut Ui;
        let initial_scale = GetDpiForSystem() as f32 / 96.0;
        app.scale = initial_scale;
        let work = work_area();
        if let Some(anchor) = app.state.dock {
            let point = POINT {
                x: app
                    .state
                    .x
                    .unwrap_or(work.left)
                    .saturating_add((docking::ICON_SIZE * initial_scale / 2.0) as i32),
                y: app
                    .state
                    .y
                    .unwrap_or(work.top)
                    .saturating_add((docking::ICON_SIZE * initial_scale / 2.0) as i32),
            };
            let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
            app.dock = Some(Dock::new(Layout::new(
                anchor,
                app.state.width,
                app.state.height,
                initial_scale,
                monitor_work_area(monitor),
            )));
        }
        let width = if app.dock.is_some() {
            docking::ICON_SIZE as i32
        } else if app.state.collapsed {
            288
        } else {
            DEFAULT_WIDTH
        };
        let height = if app.dock.is_some() {
            docking::ICON_SIZE as i32
        } else if app.state.collapsed {
            86
        } else {
            DEFAULT_HEIGHT
        };
        // Start with a bounded canvas, then restore against the actual monitor/DPI.
        // A stale or edited settings file must never allocate an enormous bitmap.
        let restored_width = if app.state.collapsed {
            288
        } else {
            app.state.width
        }
        .min(((work.right - work.left) as f32 / initial_scale) as i32);
        let x = app
            .dock
            .as_ref()
            .map(|dock| dock.layout.icon.left)
            .unwrap_or_else(|| {
                app.state
                    .x
                    .unwrap_or(work.right - (restored_width as f32 * initial_scale) as i32 - 40)
            });
        let y = app
            .dock
            .as_ref()
            .map(|dock| dock.layout.icon.top)
            .unwrap_or_else(|| app.state.y.unwrap_or(work.top + 64));
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED
                | WS_EX_APPWINDOW
                | if app.state.pinned {
                    WS_EX_TOPMOST
                } else {
                    WINDOW_EX_STYLE(0)
                },
            PCWSTR(class.as_ptr()),
            w!("玻璃待办"),
            WS_POPUP | WS_THICKFRAME,
            x,
            y,
            (width as f32 * initial_scale) as i32,
            (height as f32 * initial_scale) as i32,
            None,
            None,
            Some(instance.into()),
            Some(pointer.cast()),
        )?;
        app.hwnd = hwnd;
        app.scale = GetDpiForWindow(hwnd) as f32 / 96.0;
        core.hwnd.store(hwnd.0 as isize, Ordering::Release);
        if app.dock.is_some() {
            app.restore_dock();
        } else {
            let (minimum, maximum) = track_limits(
                app.scale,
                app.state.collapsed,
                false,
                window_work_area(hwnd),
            );
            let restored_width = if app.state.collapsed {
                288
            } else {
                app.state.width
            };
            let restored_height = if app.state.collapsed {
                86
            } else {
                app.state.height
            };
            let _ = SetWindowPos(
                hwnd,
                None,
                0,
                0,
                ((restored_width as f32 * app.scale).round() as i32).clamp(minimum.x, maximum.x),
                ((restored_height as f32 * app.scale).round() as i32).clamp(minimum.y, maximum.y),
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        app.ensure_on_screen();
        app.create_backdrop(instance.into());
        app.tray(true);
        // Honor a launcher's explicit hidden startup, including resource checks.
        // Win32's implicit first-ShowWindow override can be consumed by a helper.
        let mut startup = windows::Win32::System::Threading::STARTUPINFOW::default();
        windows::Win32::System::Threading::GetStartupInfoW(&mut startup);
        if startup.dwFlags & windows::Win32::System::Threading::STARTF_USESHOWWINDOW
            == windows::Win32::System::Threading::STARTUPINFOW_FLAGS(0)
            || startup.wShowWindow != SW_HIDE.0 as u16
        {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        app.repaint();
        cfg.log(&format!(
            "窗口启动: visible={}, dpi={}",
            IsWindowVisible(hwnd).as_bool(),
            GetDpiForWindow(hwnd)
        ));
        if let Some(seconds) = cfg.exit_after {
            SetTimer(
                Some(hwnd),
                EXIT_TIMER,
                (seconds * 1000).min(u32::MAX as u64) as u32,
                None,
            );
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        core.hwnd.store(0, Ordering::Release);
        app.tray(false);
        app.save();
        if !app.backdrop.is_invalid() {
            let _ = DestroyWindow(app.backdrop);
        }
        Ok(())
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            (*(cs.lpCreateParams as *mut Ui)).hwnd = hwnd;
            let dpi = GetDpiForWindow(hwnd);
            if dpi != 0 {
                (*(cs.lpCreateParams as *mut Ui)).scale = dpi as f32 / 96.0;
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Ui;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        // Native move/resize runs a nested message loop. Enter it without
        // borrowing application state so WM_MOVE/WM_SIZE can update normally.
        if msg == WM_NCLBUTTONDOWN
            || (msg == WM_SYSCOMMAND && matches!(wparam.0 & 0xfff0, 0xf000 | 0xf010))
        {
            if !(*ptr).dispatching {
                let moving = (msg == WM_NCLBUTTONDOWN && wparam.0 == HTCAPTION as usize)
                    || (msg == WM_SYSCOMMAND && wparam.0 & 0xfff0 == 0xf010);
                (*ptr).begin_window_drag(moving);
            }
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        // Win32 can dispatch synchronously inside SetWindowPos/SendMessage. Defer
        // stateful handling until the outer borrow ends instead of aliasing &mut Ui.
        if (*ptr).dispatching {
            return match msg {
                WM_REFRESH => {
                    (*ptr).deferred_refresh = true;
                    LRESULT(0)
                }
                WM_NCCALCSIZE => LRESULT(0),
                WM_ERASEBKGND => LRESULT(1),
                // Programmatic animation bounds are already constrained. Do not
                // let the standard frame minimum enlarge the 48-DIP edge icon.
                WM_GETMINMAXINFO => {
                    let info = &mut *(lparam.0 as *mut MINMAXINFO);
                    info.ptMinTrackSize = POINT { x: 1, y: 1 };
                    LRESULT(0)
                }
                WM_SIZE | WM_MOVE => {
                    let _ = PostMessageW(Some(hwnd), WM_REFRESH, WPARAM(0), LPARAM(0));
                    LRESULT(0)
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    LRESULT(0)
                }
                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            };
        }
        (*ptr).dispatching = true;
        let app = &mut *ptr;
        let result = match msg {
            WM_APPEARANCE => {
                app.appearance = app.core.appearance();
                app.dark = app.appearance.theme.dark(is_dark());
                let result = app.painter.set_text_size(app.appearance.text_size);
                app.report(result);
                if IsWindow(Some(app.settings)).as_bool() {
                    let _ = PostMessageW(Some(app.settings), WM_APPEARANCE, WPARAM(0), LPARAM(0));
                }
                app.repaint();
                LRESULT(0)
            }
            WM_NCCALCSIZE => LRESULT(0),
            WM_PAINT => {
                app.repaint();
                let _ = ValidateRect(Some(hwnd), None);
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_REFRESH => {
                app.refresh();
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let p = POINT {
                    x: xparam(lparam),
                    y: yparam(lparam),
                };
                app.mouse_down(p);
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                app.dock_pointer_enter(false);
                app.hover_at(Some((
                    xparam(lparam) as f32 / app.scale,
                    yparam(lparam) as f32 / app.scale,
                )));
                if let Some((_, start)) = app.pressed {
                    let p = POINT {
                        x: xparam(lparam),
                        y: yparam(lparam),
                    };
                    if (p.x - start.x).abs() + (p.y - start.y).abs() > 5 {
                        app.dragging = true;
                        app.drag_y = p.y as f32 / app.scale;
                        app.repaint();
                    }
                }
                LRESULT(0)
            }
            WM_NCMOUSEMOVE => {
                app.dock_pointer_enter(true);
                LRESULT(0)
            }
            WM_MOUSELEAVE | WM_NCMOUSELEAVE => {
                app.hover_at(None);
                app.dock_pointer_leave(cursor_position());
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                app.mouse_up(POINT {
                    x: xparam(lparam),
                    y: yparam(lparam),
                });
                LRESULT(0)
            }
            WM_LBUTTONDBLCLK => {
                if let Some(Action::Row(id)) = app.action_at(
                    xparam(lparam) as f32 / app.scale,
                    yparam(lparam) as f32 / app.scale,
                ) {
                    app.open_editor(Some(id));
                }
                LRESULT(0)
            }
            WM_CAPTURECHANGED => {
                app.pressed = None;
                app.pending_delete = None;
                app.dragging = false;
                app.repaint();
                app.arm_dock_hide();
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                let delta = (wparam.0 >> 16) as u16 as i16 as f32;
                let mut point = POINT {
                    x: xparam(lparam),
                    y: yparam(lparam),
                };
                let _ = ScreenToClient(hwnd, &mut point);
                if app
                    .completed_area
                    .has(point.x as f32 / app.scale, point.y as f32 / app.scale)
                {
                    app.done_scroll = (app.done_scroll - delta / 120.0 * 36.0).max(0.0);
                } else {
                    app.scroll = (app.scroll - delta / 120.0 * 48.0).max(0.0);
                }
                app.repaint();
                LRESULT(0)
            }
            WM_CONTEXTMENU => {
                let mut p = POINT {
                    x: xparam(lparam),
                    y: yparam(lparam),
                };
                if p.x == -1 {
                    let _ = GetCursorPos(&mut p);
                }
                app.menu(p);
                LRESULT(0)
            }
            WM_KEYDOWN => {
                let key = wparam.0 as u16;
                let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
                if ctrl && key == VK_OEM_COMMA.0 {
                    app.open_settings();
                } else if ctrl && key == b'N' as u16 {
                    app.open_editor(None)
                } else if ctrl && key == b'Z' as u16 {
                    let result = app.undo_delete();
                    app.report(result);
                } else if key == VK_F2.0 {
                    if let Some(id) = app.selected {
                        app.open_editor(Some(id));
                    }
                } else if key == VK_DELETE.0
                    && !app.state.collapsed
                    && let Some(id) = app.selected
                {
                    let result = app.delete_todo(id, None);
                    app.report(result);
                }
                LRESULT(0)
            }
            WM_NCHITTEST => {
                let mut p = POINT {
                    x: xparam(lparam),
                    y: yparam(lparam),
                };
                let _ = ScreenToClient(hwnd, &mut p);
                let mut r = RECT::default();
                let _ = GetClientRect(hwnd, &mut r);
                LRESULT(app.hit_test(
                    p.x as f32 / app.scale,
                    p.y as f32 / app.scale,
                    r.right as f32 / app.scale,
                    r.bottom as f32 / app.scale,
                ) as isize)
            }
            WM_GETMINMAXINFO => {
                let info = &mut *(lparam.0 as *mut MINMAXINFO);
                (info.ptMinTrackSize, info.ptMaxTrackSize) = track_limits(
                    app.scale,
                    app.state.collapsed,
                    app.motion.fold_size.is_some(),
                    window_work_area(hwnd),
                );
                if app.dock.is_some() {
                    let size = (docking::ICON_SIZE * app.scale).round() as i32;
                    info.ptMinTrackSize = POINT { x: size, y: size };
                }
                LRESULT(0)
            }
            WM_SIZE => {
                app.sync_backdrop();
                app.repaint();
                LRESULT(0)
            }
            WM_MOVE => {
                app.sync_backdrop();
                SetTimer(Some(hwnd), SAVE_TIMER, 400, None);
                LRESULT(0)
            }
            WM_EXITSIZEMOVE => {
                app.end_window_drag();
                app.save();
                app.sync_backdrop();
                app.repaint();
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == MOTION_TIMER {
                    app.animate();
                } else if wparam.0 == DOCK_HIDE_TIMER {
                    app.dock_hide_timeout(Instant::now());
                } else if wparam.0 == SAVE_TIMER {
                    let _ = KillTimer(Some(hwnd), SAVE_TIMER);
                    app.save();
                } else if wparam.0 == EXIT_TIMER {
                    app.quit();
                }
                LRESULT(0)
            }
            WM_DPICHANGED => {
                app.scale = GetDpiForWindow(hwnd) as f32 / 96.0;
                let r = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                app.ensure_on_screen();
                app.sync_backdrop();
                app.repaint();
                LRESULT(0)
            }
            WM_SETTINGCHANGE => {
                app.dark = app.appearance.theme.dark(is_dark());
                app.motion.enabled = animations_enabled();
                if IsWindow(Some(app.settings)).as_bool() {
                    let _ =
                        PostMessageW(Some(app.settings), WM_SETTINGCHANGE, WPARAM(0), LPARAM(0));
                }
                app.ensure_on_screen();
                app.repaint();
                LRESULT(0)
            }
            WM_DISPLAYCHANGE => {
                app.ensure_on_screen();
                app.sync_backdrop();
                LRESULT(0)
            }
            WM_QUERYENDSESSION => {
                app.save();
                let _ = app.core.save_appearance(&app.core.appearance());
                LRESULT(1)
            }
            WM_CLOSE => {
                app.save();
                let _ = ShowWindow(hwnd, SW_HIDE);
                if !app.backdrop.is_invalid() {
                    let _ = ShowWindow(app.backdrop, SW_HIDE);
                }
                LRESULT(0)
            }
            WM_TRAY => {
                if lparam.0 as u32 == WM_LBUTTONUP {
                    app.set_dock_open(true, false);
                    let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
                    let _ = SetForegroundWindow(hwnd);
                    app.sync_backdrop();
                } else if lparam.0 as u32 == WM_RBUTTONUP {
                    let mut p = POINT::default();
                    let _ = GetCursorPos(&mut p);
                    app.menu(p);
                }
                LRESULT(0)
            }
            WM_ACTIVATE | WM_ENABLE | WM_EXITMENULOOP => {
                app.arm_dock_hide();
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        };
        (*ptr).dispatching = false;
        if std::mem::take(&mut (*ptr).deferred_refresh) {
            let _ = PostMessageW(Some(hwnd), WM_REFRESH, WPARAM(0), LPARAM(0));
        }
        result
    }
}

impl Ui {
    unsafe fn place_bounds(&mut self, bounds: RECT) {
        unsafe {
            if self.hwnd.is_invalid() {
                return;
            }
            let mut current = RECT::default();
            if GetWindowRect(self.hwnd, &mut current).is_ok() && current == bounds {
                return;
            }
            let dispatching = self.dispatching;
            self.dispatching = true;
            let _ = SetWindowPos(
                self.hwnd,
                None,
                bounds.left,
                bounds.top,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            self.dispatching = dispatching;
        }
    }
    unsafe fn apply_dock_bounds(&mut self) {
        unsafe {
            if let Some(dock) = &self.dock {
                self.place_bounds(dock.bounds());
            }
        }
    }
    unsafe fn restore_dock(&mut self) {
        unsafe {
            let Some(anchor) = self.state.dock else {
                return;
            };
            self.state.collapsed = false;
            self.motion.fold.snap(0.0);
            self.motion.fold_size = None;
            self.dock = Some(Dock::new(Layout::new(
                anchor,
                self.state.width,
                self.state.height,
                self.scale,
                window_work_area(self.hwnd),
            )));
            self.apply_dock_bounds();
        }
    }
    unsafe fn detach_dock(&mut self) {
        unsafe {
            self.cancel_dock_hide();
            if let Some(dock) = self.dock.take() {
                self.state.dock = None;
                self.state.collapsed = false;
                self.motion.fold.snap(0.0);
                self.place_bounds(dock.layout.expanded);
                self.save();
            }
        }
    }
    unsafe fn begin_window_drag(&mut self, moving: bool) {
        unsafe {
            if !self.native_drag {
                self.detach_dock();
            }
            self.native_drag = true;
            self.moving_window = moving;
        }
    }
    unsafe fn end_window_drag(&mut self) {
        unsafe {
            self.end_window_drag_at(cursor_position());
        }
    }
    unsafe fn end_window_drag_at(&mut self, position: Option<POINT>) {
        unsafe {
            let moving = self.native_drag && self.moving_window;
            self.native_drag = false;
            self.moving_window = false;
            if !moving {
                return;
            }
            self.save();
            let mut bounds = RECT::default();
            if GetWindowRect(self.hwnd, &mut bounds).is_err() {
                return;
            }
            let work = window_work_area(self.hwnd);
            if let Some(anchor) = docking::detect(bounds, work, self.scale) {
                self.state.dock = Some(anchor);
                self.state.collapsed = false;
                self.motion.fold.snap(0.0);
                self.motion.fold_size = None;
                let layout = Layout::new(
                    anchor,
                    self.state.width,
                    self.state.height,
                    self.scale,
                    work,
                );
                let dock = Dock::from_expanded(layout, bounds, self.motion.enabled);
                self.dock = Some(dock);
                self.apply_dock_bounds();
                // Register leave tracking immediately. If the release point is
                // already outside the final icon, its next entry is intentional.
                self.dock_pointer_move(false, position);
                self.schedule_motion();
            } else {
                self.ensure_on_screen();
            }
        }
    }
    unsafe fn set_dock_open(&mut self, open: bool, animate: bool) {
        unsafe {
            let Some(dock) = &mut self.dock else {
                return;
            };
            if dock.open == open && (animate || !dock.active()) {
                return;
            }
            dock.set_open(open, animate && self.motion.enabled);
            if open {
                self.start_dock_hide();
            } else {
                self.cancel_dock_hide();
            }
            self.apply_dock_bounds();
            self.repaint();
            self.schedule_motion();
        }
    }
    unsafe fn dock_pointer_enter(&mut self, nonclient: bool) {
        unsafe {
            if self.dock.is_none() || self.native_drag {
                return;
            }
            self.dock_pointer_move(nonclient, cursor_position());
        }
    }
    unsafe fn dock_pointer_move(&mut self, nonclient: bool, position: Option<POINT>) {
        unsafe {
            if self.dock.is_none() || self.native_drag {
                return;
            }
            let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE
                    | if nonclient {
                        TME_NONCLIENT
                    } else {
                        TRACKMOUSEEVENT_FLAGS(0)
                    },
                hwndTrack: self.hwnd,
                dwHoverTime: 0,
            });
            let Some(point) = position else {
                return;
            };
            let mut bounds = RECT::default();
            let over_window =
                GetWindowRect(self.hwnd, &mut bounds).is_ok() && docking::contains(bounds, point);
            if over_window {
                self.cancel_dock_hide();
            } else {
                self.start_dock_hide();
            }
            if self
                .dock
                .as_mut()
                .is_some_and(|dock| dock.wants_hover(point, over_window))
            {
                self.set_dock_open(true, true);
            }
        }
    }
    unsafe fn dock_pointer_leave(&mut self, position: Option<POINT>) {
        unsafe {
            if let (Some(dock), Some(point)) = (&mut self.dock, position) {
                dock.observe_pointer(point);
            }
            // Leave can be reported in the transparent margin, before the
            // pointer crosses the native rectangle. Always validate it later.
            self.start_dock_hide();
        }
    }
    unsafe fn pointer_in_window_at(&self, position: Option<POINT>) -> bool {
        unsafe {
            let mut bounds = RECT::default();
            position.is_some_and(|point| {
                GetWindowRect(self.hwnd, &mut bounds).is_ok() && docking::contains(bounds, point)
            })
        }
    }
    unsafe fn pointer_over_note_at(&self, position: Option<POINT>) -> bool {
        unsafe {
            if !self.pointer_in_window_at(position) {
                return false;
            }
            let hit = WindowFromPoint(position.unwrap());
            hit == self.hwnd || IsChild(self.hwnd, hit).as_bool()
        }
    }
    unsafe fn cancel_dock_hide(&mut self) {
        unsafe {
            if self.dock_hide_due.take().is_some() {
                let _ = KillTimer(Some(self.hwnd), DOCK_HIDE_TIMER);
            }
        }
    }
    unsafe fn start_dock_hide(&mut self) {
        unsafe {
            if self.dock_hide_due.is_none()
                && self.dock.as_ref().is_some_and(|dock| dock.open)
                && !self.native_drag
                && SetTimer(
                    Some(self.hwnd),
                    DOCK_HIDE_TIMER,
                    docking::HIDE_DELAY_MS,
                    None,
                ) != 0
            {
                self.dock_hide_due = Some(
                    Instant::now()
                        + std::time::Duration::from_millis(u64::from(docking::HIDE_DELAY_MS)),
                );
            }
        }
    }
    unsafe fn dock_hide_timeout(&mut self, now: Instant) {
        unsafe {
            let Some(due) = self.dock_hide_due else {
                return;
            };
            if now < due {
                // A canceled timer message can already be queued. It must not
                // consume the newer leave deadline or close a re-entered preview.
                let remaining = due
                    .duration_since(now)
                    .as_millis()
                    .saturating_add(1)
                    .min(u32::MAX as u128) as u32;
                SetTimer(Some(self.hwnd), DOCK_HIDE_TIMER, remaining.max(1), None);
                return;
            }
            self.check_dock_hide();
        }
    }
    unsafe fn arm_dock_hide(&mut self) {
        unsafe {
            self.arm_dock_hide_at(cursor_position());
        }
    }
    unsafe fn arm_dock_hide_at(&mut self, position: Option<POINT>) {
        unsafe {
            if self.dock.as_ref().is_some_and(|dock| dock.open)
                && !self.pointer_over_note_at(position)
            {
                self.start_dock_hide();
            }
        }
    }
    fn dock_tracking_flags(&self, point: POINT) -> TRACKMOUSEEVENT_FLAGS {
        unsafe {
            let mut bounds = RECT::default();
            if GetWindowRect(self.hwnd, &mut bounds).is_err() {
                return TME_LEAVE;
            }
            let scale = self.scale;
            let hit = self.hit_test(
                (point.x - bounds.left) as f32 / scale,
                (point.y - bounds.top) as f32 / scale,
                (bounds.right - bounds.left) as f32 / scale,
                (bounds.bottom - bounds.top) as f32 / scale,
            );
            TME_LEAVE
                | if hit == HTCLIENT {
                    TRACKMOUSEEVENT_FLAGS(0)
                } else {
                    TME_NONCLIENT
                }
        }
    }
    unsafe fn track_dock_pointer(&self, point: POINT) {
        unsafe {
            let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: self.dock_tracking_flags(point),
                hwndTrack: self.hwnd,
                dwHoverTime: 0,
            });
        }
    }
    unsafe fn check_dock_hide(&mut self) {
        unsafe {
            self.cancel_dock_hide();
            if !self.dock.as_ref().is_some_and(|dock| dock.open) {
                return;
            }
            let position = cursor_position();
            if self.pointer_over_note_at(position) {
                // A growing panel can arrive under a stationary pointer after its
                // earlier leave notification. Re-arm the correct tracking area.
                if let Some(point) = position {
                    self.track_dock_pointer(point);
                }
                return;
            }
            if self.native_drag
                || self.dragging
                || self.pressed.is_some()
                || GetCapture() == self.hwnd
                || IsWindow(Some(self.editor)).as_bool()
                || IsWindow(Some(self.settings)).as_bool()
                || IsWindow(Some(self.status_picker)).as_bool()
            {
                return;
            }
            self.set_dock_open(false, true);
        }
    }
    fn hit_test(&self, x: f32, y: f32, w: f32, h: f32) -> u32 {
        if self.dock.as_ref().is_some_and(|dock| !dock.interactive()) {
            return HTCLIENT;
        }
        if self.action_at(x, y).is_some() {
            return HTCLIENT;
        }
        if !self.state.collapsed && self.motion.fold_size.is_none() {
            let left = x < RESIZE_EDGE;
            let right = x >= w - RESIZE_EDGE;
            let top = y < RESIZE_EDGE;
            let bottom = y >= h - RESIZE_EDGE;
            if x < RESIZE_CORNER && y < RESIZE_CORNER && (left || top) {
                return HTTOPLEFT;
            }
            if x >= w - RESIZE_CORNER && y < RESIZE_CORNER && (right || top) {
                return HTTOPRIGHT;
            }
            if x < RESIZE_CORNER && y >= h - RESIZE_CORNER && (left || bottom) {
                return HTBOTTOMLEFT;
            }
            if x >= w - RESIZE_CORNER && y >= h - RESIZE_CORNER && (right || bottom) {
                return HTBOTTOMRIGHT;
            }
            if left {
                return HTLEFT;
            }
            if right {
                return HTRIGHT;
            }
            if top {
                return HTTOP;
            }
            if bottom {
                return HTBOTTOM;
            }
        }
        if (8.0..61.0).contains(&y) {
            HTCAPTION
        } else {
            HTCLIENT
        }
    }
    unsafe fn refresh(&mut self) {
        unsafe {
            self.core.refresh_started();
            if let Ok(todos) = self.core.list() {
                for row in self.motion.rows.values_mut() {
                    if todos
                        .iter()
                        .any(|t| t.id == row.todo.id && t.status == Status::Done)
                    {
                        row.check.set(1.0);
                        row.progress.set(0.0);
                    }
                }
                if self.reveal_todo.is_none()
                    && todos.iter().any(|t| {
                        t.status != Status::Done && !self.todos.iter().any(|old| old.id == t.id)
                    })
                {
                    self.scroll = f32::MAX;
                }
                self.todos = todos;
                if self
                    .selected
                    .is_some_and(|id| !self.todos.iter().any(|t| t.id == id))
                {
                    self.selected = None;
                }
            }
            if let Ok(available) = self.core.can_undo_delete() {
                self.can_undo_delete = available;
            }
            for child in [self.settings, self.status_picker] {
                if IsWindow(Some(child)).as_bool() {
                    let _ = PostMessageW(Some(child), WM_REFRESH, WPARAM(0), LPARAM(0));
                }
            }
            self.sync_backdrop();
            self.repaint();
            self.arm_dock_hide();
        }
    }
    fn delete_todo(&mut self, id: i64, version: Option<i64>) -> Result<()> {
        let version = version.or_else(|| self.todos.iter().find(|t| t.id == id).map(|t| t.version));
        self.core.delete(id, version)?;
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.pressed = None;
        self.dragging = false;
        Ok(())
    }
    unsafe fn undo_delete(&mut self) -> Result<()> {
        unsafe {
            let todo = self.core.undo()?;
            self.selected = Some(todo.id);
            self.reveal_todo = Some(todo.id);
            if todo.status == Status::Done && !self.state.completed_open {
                self.state.completed_open = true;
                self.motion.completed.set(1.0);
                self.save();
            }
            Ok(())
        }
    }
    unsafe fn report(&self, result: Result<()>) {
        unsafe {
            if let Err(e) = result {
                self.cfg.log(&e.to_string());
                let msg = wide(&e.to_string());
                MessageBoxW(
                    Some(self.hwnd),
                    PCWSTR(msg.as_ptr()),
                    w!("玻璃待办"),
                    MB_OK | MB_ICONINFORMATION,
                );
            }
        }
    }
    unsafe fn save(&mut self) {
        unsafe {
            let mut r = RECT::default();
            if GetWindowRect(self.hwnd, &mut r).is_ok() {
                let anchor = self.dock.as_ref().map(|dock| dock.layout.icon).unwrap_or(r);
                self.state.x = Some(anchor.left);
                self.state.y = Some(anchor.top);
                if self.dock.is_none() && !self.state.collapsed && self.motion.fold_size.is_none() {
                    self.state.width = (((r.right - r.left) as f32 / self.scale).round() as i32)
                        .max(DEFAULT_WIDTH);
                    self.state.height = (((r.bottom - r.top) as f32 / self.scale).round() as i32)
                        .max(DEFAULT_HEIGHT);
                }
                if let Err(e) = self.core.save_window(&self.state) {
                    self.cfg.log(&e.to_string());
                }
            }
        }
    }
    unsafe fn ensure_on_screen(&mut self) {
        unsafe {
            let mut r = RECT::default();
            if GetWindowRect(self.hwnd, &mut r).is_err() {
                return;
            }
            let work = window_work_area(self.hwnd);
            if let (Some(anchor), Some(dock)) = (self.state.dock, &mut self.dock) {
                dock.layout = Layout::new(
                    anchor,
                    self.state.width,
                    self.state.height,
                    self.scale,
                    work,
                );
                dock.set_open(dock.open, false);
                self.apply_dock_bounds();
                return;
            }
            let (minimum, maximum) = track_limits(
                self.scale,
                self.state.collapsed,
                self.motion.fold_size.is_some(),
                work,
            );
            let w = (r.right - r.left).clamp(minimum.x, maximum.x);
            let h = (r.bottom - r.top).clamp(minimum.y, maximum.y);
            let x = r.left.clamp(work.left, (work.right - w).max(work.left));
            let y = r.top.clamp(work.top, (work.bottom - h).max(work.top));
            let _ = SetWindowPos(self.hwnd, None, x, y, w, h, SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }
    unsafe fn repaint(&mut self) {
        unsafe {
            if self.hwnd.is_invalid() {
                return;
            }
            let mut r = RECT::default();
            if GetClientRect(self.hwnd, &mut r).is_err() || r.right < 1 || r.bottom < 1 {
                return;
            }
            let w = r.right as f32 / self.scale;
            let h = r.bottom as f32 / self.scale;
            if !self.motion.enabled {
                self.motion.step(5.0);
            }
            if let Err(e) = self.painter.begin(r.right, r.bottom, self.scale) {
                self.cfg.log(&format!("绘制失败: {e}"));
                return;
            }
            self.draw(w, h);
            if let Err(e) = self.painter.end(self.hwnd) {
                self.cfg.log(&format!("更新窗口图像失败: {e:#}"));
            }
            self.schedule_motion();
        }
    }
    unsafe fn schedule_motion(&mut self) {
        unsafe {
            if self.hwnd.is_invalid() {
                return;
            }
            if self.motion.active()
                || self.motion.fold_size.is_some()
                || self.dock.as_ref().is_some_and(Dock::active)
            {
                if !self.motion.timer {
                    self.motion.timer = true;
                    self.motion.last = Instant::now();
                    SetTimer(Some(self.hwnd), MOTION_TIMER, 16, None);
                }
            } else if self.motion.timer {
                let _ = KillTimer(Some(self.hwnd), MOTION_TIMER);
                self.motion.timer = false;
            }
        }
    }
    unsafe fn animate(&mut self) {
        unsafe {
            let now = Instant::now();
            let dt = now.duration_since(self.motion.last).as_secs_f32();
            self.motion.step(dt);
            self.motion.last = now;
            let mut preview_settled = false;
            if let Some(dock) = &mut self.dock {
                let was_active = dock.active();
                dock.step(if self.motion.enabled { dt } else { 5.0 });
                preview_settled = was_active && !dock.active() && dock.open;
                self.apply_dock_bounds();
            }
            if preview_settled {
                let position = cursor_position();
                if self.pointer_over_note_at(position) {
                    if let Some(point) = position {
                        self.track_dock_pointer(point);
                    }
                } else {
                    self.start_dock_hide();
                }
            }
            if self.motion.fold_size.is_some() && !self.motion.fold.active() {
                self.motion.fold_size = None;
                let (w, h) = if self.state.collapsed {
                    (288, 86)
                } else {
                    (self.state.width, self.state.height)
                };
                let _ = SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    (w as f32 * self.scale) as i32,
                    (h as f32 * self.scale) as i32,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
                self.ensure_on_screen();
                self.save();
            }
            self.sync_backdrop();
            self.repaint();
        }
    }
    unsafe fn hover_at(&mut self, point: Option<(f32, f32)>) {
        unsafe {
            let action = point.and_then(|(x, y)| self.action_at(x, y));
            if action != self.motion.hovered {
                self.motion.old_hover = self.motion.hovered;
                self.motion.hovered = action;
                self.motion.hover.snap(0.0);
                self.motion.hover.set(1.0);
                self.repaint();
            }
            if point.is_some() {
                let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: self.hwnd,
                    dwHoverTime: 0,
                });
            }
        }
    }
    unsafe fn draw(&mut self, width: f32, height: f32) {
        unsafe {
            if let Some(dock) = &self.dock {
                let reveal = dock.reveal.value.clamp(0.0, 1.0);
                let interactive = dock.interactive();
                let full_width =
                    (dock.layout.expanded.right - dock.layout.expanded.left) as f32 / self.scale;
                let full_height =
                    (dock.layout.expanded.bottom - dock.layout.expanded.top) as f32 / self.scale;
                self.hits.clear();
                self.completed_area = rect(0.0, 0.0, 0.0, 0.0);
                if reveal > 0.001 {
                    self.painter.group_opacity(reveal);
                    self.painter.view(
                        self.scale * width / full_width,
                        self.scale * height / full_height,
                    );
                    self.draw_note(full_width, full_height);
                    self.painter.view(self.scale, self.scale);
                }
                self.painter.group_opacity(1.0);
                if reveal < 0.999 {
                    self.draw_dock_icon(width, height, 1.0 - reveal);
                }
                if !interactive {
                    self.hits.clear();
                }
                return;
            }
            self.draw_note(width, height);
        }
    }
    unsafe fn draw_dock_icon(&self, width: f32, height: f32, opacity: f32) {
        unsafe {
            let p = &self.painter;
            p.opacity(opacity);
            let ink = if self.dark {
                color(0.65, 0.83, 1.0, 1.0)
            } else {
                color(0.12, 0.39, 0.68, 1.0)
            };
            p.fill(
                rect(0.0, 0.0, width, height),
                0.0,
                color(0.0, 0.0, 0.0, 1.0 / 255.0),
            );
            p.fill(
                rect(2.0, 2.0, width - 4.0, height - 4.0),
                15.0,
                if self.dark {
                    color(0.10, 0.15, 0.22, 0.96)
                } else {
                    color(0.94, 0.97, 1.0, 0.96)
                },
            );
            p.border(
                rect(2.5, 2.5, width - 5.0, height - 5.0),
                14.5,
                color(1.0, 1.0, 1.0, 0.85),
                1.2,
            );
            let (cx, cy) = (width / 2.0, height / 2.0);
            p.fill(
                rect(cx - 10.0, cy - 11.0, 20.0, 23.0),
                5.0,
                color(0.3, 0.6, 0.9, 0.10),
            );
            p.line(cx - 7.0, cy - 3.0, cx - 4.0, cy, ink, 1.8);
            p.line(cx - 4.0, cy, cx + 1.0, cy - 6.0, ink, 1.8);
            p.line(cx + 4.0, cy - 4.0, cx + 8.0, cy - 4.0, ink, 1.5);
            p.line(cx - 7.0, cy + 6.0, cx + 8.0, cy + 6.0, ink, 1.5);
            if self.todos.iter().any(|todo| todo.status != Status::Done) {
                p.circle(cx + 12.0, cy - 12.0, 3.0, ink, true);
            }
            p.opacity(1.0);
        }
    }
    unsafe fn draw_note(&mut self, width: f32, height: f32) {
        unsafe {
            let (ew, eh) = self.motion.fold_size.unwrap_or((width, height));
            let fold = if self.motion.fold_size.is_some() {
                self.motion.fold.value
            } else if self.state.collapsed {
                1.0
            } else {
                0.0
            };
            let w = if self.motion.fold_size.is_some() {
                ew + (288.0 - ew) * fold
            } else {
                width
            };
            let h = if self.motion.fold_size.is_some() {
                eh + (86.0 - eh) * fold
            } else {
                height
            };
            let body_h = if self.motion.fold_size.is_some() {
                eh
            } else {
                height
            };
            self.hits.clear();
            self.completed_area = rect(0.0, 0.0, 0.0, 0.0);
            let p = &self.painter;
            if !self.state.collapsed && self.motion.fold_size.is_none() {
                // Alpha-zero pixels in layered windows pass mouse hits through.
                // One alpha step keeps the rounded corner grips usable invisibly.
                for (x, y) in [
                    (0.0, 0.0),
                    (w - RESIZE_CORNER, 0.0),
                    (0.0, h - RESIZE_CORNER),
                    (w - RESIZE_CORNER, h - RESIZE_CORNER),
                ] {
                    p.fill(
                        rect(x, y, RESIZE_CORNER, RESIZE_CORNER),
                        0.0,
                        color(0.0, 0.0, 0.0, 1.0 / 255.0),
                    );
                }
            }
            let ink = if self.dark {
                color(0.94, 0.96, 1.0, 1.0)
            } else {
                color(0.07, 0.12, 0.18, 1.0)
            };
            let muted = if self.dark {
                color(0.76, 0.82, 0.9, 1.0)
            } else {
                color(0.20, 0.27, 0.35, 1.0)
            };
            let accent = if self.dark {
                color(0.55, 0.79, 1.0, 1.0)
            } else {
                color(0.09, 0.31, 0.54, 1.0)
            };
            let radius = (self.appearance.radius as f32 - 6.0 * fold).max(8.0);
            p.fill(
                rect(9.0, 12.0, w - 18.0, h - 18.0),
                radius,
                color(0.06, 0.12, 0.2, 0.15),
            );
            p.fill(
                rect(8.0, 8.0, w - 16.0, h - 16.0),
                radius,
                if self.dark {
                    color(0.07, 0.10, 0.15, self.appearance.opacity as f32 / 100.0)
                } else {
                    color(0.96, 0.98, 1.0, self.appearance.opacity as f32 / 100.0)
                },
            );
            p.border(
                rect(8.7, 8.7, w - 17.4, h - 17.4),
                radius - 0.7,
                color(1.0, 1.0, 1.0, 0.86),
                1.4,
            );
            p.border(
                rect(11.0, 11.0, w - 22.0, h - 22.0),
                radius - 2.0,
                color(0.86, 0.95, 1.0, 0.48),
                2.0,
            );
            p.border(
                rect(13.0, 13.0, w - 26.0, h - 26.0),
                radius - 4.0,
                color(0.12, 0.24, 0.34, 0.13),
                1.0,
            );
            let active: Vec<_> = self
                .todos
                .iter()
                .filter(|t| t.status != Status::Done)
                .collect();
            let done: Vec<_> = self
                .todos
                .iter()
                .filter(|t| t.status == Status::Done)
                .collect();
            for x in [25.0, 29.0] {
                for y in [28.0, 32.0, 36.0] {
                    p.circle(x, y, 0.8, muted, true);
                }
            }
            p.opacity(1.0 - fold);
            p.text("待办", rect(39.0, 23.0, 52.0, 30.0), 2, ink);
            p.text(
                &active.len().to_string(),
                rect(99.0, 28.0, 30.0, 23.0),
                0,
                muted,
            );
            p.opacity(fold);
            p.text(
                &format!("待办  {}", active.len()),
                rect(38.0, 19.0, w - 156.0, 21.0),
                0,
                muted,
            );
            p.text(
                active
                    .first()
                    .map(|t| t.content.lines().next().unwrap_or(""))
                    .unwrap_or("全部完成"),
                rect(38.0, 41.0, w - 156.0, 25.0),
                1,
                ink,
            );
            p.opacity(1.0);
            let settings = rect(w - 107.0, 19.0, 28.0, 30.0);
            p.fill(
                settings,
                14.0,
                color(0.20, 0.40, 0.65, self.motion.feedback(Action::Settings)),
            );
            let gx = w - 93.0;
            let gy = 34.0;
            p.circle(gx, gy, 5.4, muted, false);
            p.circle(gx, gy, 1.8, muted, false);
            for i in 0..8 {
                let angle = i as f32 * std::f32::consts::FRAC_PI_4;
                let (s, c) = angle.sin_cos();
                p.line(
                    gx + c * 6.0,
                    gy + s * 6.0,
                    gx + c * 8.0,
                    gy + s * 8.0,
                    muted,
                    1.5,
                );
            }
            self.hits.push(Hit {
                r: settings,
                action: Action::Settings,
            });
            let pin = rect(w - 77.0, 19.0, 28.0, 30.0);
            p.fill(
                pin,
                14.0,
                color(
                    1.0,
                    1.0,
                    1.0,
                    if self.state.pinned { 0.18 } else { 0.0 } + self.motion.feedback(Action::Pin),
                ),
            );
            let pc = if self.state.pinned { accent } else { muted };
            p.line(w - 65.0, 27.0, w - 58.0, 27.0, pc, 1.2);
            p.line(w - 63.0, 27.0, w - 63.0, 34.0, pc, 1.2);
            p.line(w - 60.0, 27.0, w - 60.0, 34.0, pc, 1.2);
            p.line(w - 66.0, 35.0, w - 57.0, 35.0, pc, 1.2);
            p.line(w - 61.5, 35.0, w - 61.5, 41.0, pc, 1.2);
            self.hits.push(Hit {
                r: pin,
                action: Action::Pin,
            });
            let fr = rect(w - 47.0, 19.0, 28.0, 30.0);
            p.fill(
                fr,
                14.0,
                color(1.0, 1.0, 1.0, self.motion.feedback(Action::Fold)),
            );
            p.line(
                w - 38.0,
                35.0 - 3.0 * fold,
                w - 34.0,
                31.0 + 5.0 * fold,
                muted,
                1.3,
            );
            p.line(
                w - 34.0,
                31.0 + 5.0 * fold,
                w - 30.0,
                35.0 - 3.0 * fold,
                muted,
                1.3,
            );
            self.hits.push(Hit {
                r: fr,
                action: Action::Fold,
            });
            if fold < 0.999 {
                let body_alpha = (1.0 - fold).powi(2);
                p.clip(rect(18.0, 61.0, w - 36.0, (h - 80.0).max(0.0)));
                p.opacity(body_alpha);
                let done_height = (done.len() as f32 * 36.0)
                    .min((body_h - 260.0).max(0.0))
                    .min(144.0)
                    * self.motion.completed.value;
                let summary_y = body_h - 45.0 - done_height;
                let add_y = summary_y - 51.0;
                let list_top = 64.0;
                let list_bottom = add_y - 10.0;
                let mut status_widths: HashMap<&str, f32> = HashMap::new();
                for todo in &self.todos {
                    status_widths.entry(&todo.status_name).or_insert_with(|| {
                        (p.label_width(&todo.status_name) + 20.0).clamp(56.0, 128.0)
                    });
                }
                let text_width = (w
                    - 111.0
                    - active
                        .iter()
                        .map(|todo| status_widths[todo.status_name.as_str()])
                        .fold(56.0, f32::max))
                .max(64.0);
                let heights: Vec<_> = active
                    .iter()
                    .map(|t| (p.text_height(&t.content, text_width) + 22.0).max(46.0))
                    .collect();
                let total: f32 = heights.iter().sum();
                if let Some(id) = self.reveal_todo
                    && let Some(index) = active.iter().position(|t| t.id == id)
                {
                    let top: f32 = heights[..index].iter().sum();
                    let bottom = top + heights[index];
                    let viewport = list_bottom - list_top;
                    if top < self.scroll {
                        self.scroll = top;
                    } else if bottom > self.scroll + viewport {
                        self.scroll = (bottom - viewport).max(0.0);
                    }
                    self.reveal_todo = None;
                }
                self.scroll = self.scroll.min((total - (list_bottom - list_top)).max(0.0));
                self.motion.scroll.set(self.scroll);
                if !self.motion.enabled {
                    self.motion.scroll.snap(self.scroll);
                }
                for row in self.motion.rows.values_mut() {
                    row.seen = false;
                }
                let mut y = list_top - self.motion.scroll.value;
                for (todo, rh) in active.iter().zip(heights) {
                    self.motion.place(todo, y, rh);
                    y += rh;
                }
                self.completed_area = rect(23.0, summary_y + 23.0, w - 46.0, done_height);
                if let Some(id) = self.reveal_todo
                    && done_height > 1.0
                    && let Some(index) = done.iter().position(|t| t.id == id)
                {
                    self.done_scroll = index as f32 * 36.0;
                    self.reveal_todo = None;
                }
                self.done_scroll = self
                    .done_scroll
                    .min((done.len() as f32 * 36.0 - done_height).max(0.0));
                if done_height > 0.1 {
                    let mut y = summary_y + 23.0 - self.done_scroll;
                    for todo in &done {
                        self.motion.place(todo, y, 36.0);
                        y += 36.0;
                    }
                }
                for row in self.motion.rows.values_mut() {
                    if !row.seen && !row.retiring {
                        row.retiring = true;
                        row.alpha.set(0.0);
                        row.y.set(row.y.target - 6.0);
                    }
                }
                if !self.motion.enabled {
                    self.motion.step(5.0);
                }
                for row in self.motion.rows.values() {
                    let y = row.y.value;
                    let rh = row.height.value;
                    let is_done = row.todo.status == Status::Done;
                    let (top, bottom) = if is_done {
                        (summary_y + 23.0, body_h - 17.0)
                    } else {
                        (list_top, list_bottom)
                    };
                    if row.alpha.value <= 0.0 || y + rh < top || y > bottom {
                        continue;
                    }
                    p.clip(rect(19.0, top, w - 38.0, (bottom - top).max(0.0)));
                    p.opacity(body_alpha * row.alpha.value);
                    let id = row.todo.id;
                    let hover = self
                        .motion
                        .hover(Action::Row(id))
                        .max(self.motion.hover(Action::Check(id)))
                        .max(self.motion.hover(Action::Progress(id)))
                        .max(self.motion.hover(Action::Delete(id)));
                    p.fill(
                        rect(20.0, y, w - 40.0, rh - 1.0),
                        11.0,
                        color(
                            1.0,
                            1.0,
                            1.0,
                            hover * 0.12 + if self.selected == Some(id) { 0.10 } else { 0.0 },
                        ),
                    );
                    let cy = y + if is_done { 17.0 } else { 21.0 };
                    let check = row.check.value;
                    let progress = row.progress.value;
                    let pulse = if self.motion.pressed == Some(Action::Check(id)) {
                        self.motion.pulse.value
                    } else {
                        0.0
                    };
                    p.circle(34.0, cy, 7.5 + 1.2 * pulse, muted, false);
                    p.circle(
                        34.0,
                        cy,
                        7.5 * check,
                        color(accent.r, accent.g, accent.b, check),
                        true,
                    );
                    p.circle(
                        34.0,
                        cy,
                        3.2 * progress,
                        color(accent.r, accent.g, accent.b, progress * (1.0 - check)),
                        true,
                    );
                    p.opacity(body_alpha * row.alpha.value * check);
                    p.line(30.0, cy, 33.0, cy + 3.0, color(1.0, 1.0, 1.0, 1.0), 1.4);
                    p.line(
                        33.0,
                        cy + 3.0,
                        38.0,
                        cy - 3.0,
                        color(1.0, 1.0, 1.0, 1.0),
                        1.4,
                    );
                    p.opacity(body_alpha * row.alpha.value);
                    let badge_width = status_widths
                        .get(row.todo.status_name.as_str())
                        .copied()
                        .unwrap_or_else(|| {
                            (p.label_width(&row.todo.status_name) + 20.0).clamp(56.0, 128.0)
                        });
                    p.text(
                        if is_done {
                            row.todo.content.lines().next().unwrap_or("")
                        } else {
                            &row.todo.content
                        },
                        rect(
                            51.0,
                            y + if is_done { 6.0 } else { 10.0 },
                            if is_done {
                                (w - 111.0 - badge_width).max(64.0)
                            } else {
                                text_width
                            },
                            rh - 13.0,
                        ),
                        1,
                        if is_done { muted } else { ink },
                    );
                    let status = rect(
                        w - 55.0 - badge_width,
                        y + (rh - 27.0) / 2.0,
                        badge_width,
                        27.0,
                    );
                    let (r, g, b) = row.todo.status_color.rgb(self.dark);
                    p.fill(
                        status,
                        9.0,
                        color(r, g, b, 0.12 + self.motion.feedback(Action::Progress(id))),
                    );
                    p.text(&row.todo.status_name, status, 4, color(r, g, b, 1.0));
                    if !is_done {
                        p.line(
                            24.0,
                            y + rh,
                            w - 24.0,
                            y + rh,
                            color(0.2, 0.35, 0.5, 0.14),
                            0.7,
                        );
                    }
                    let delete = rect(w - 51.0, y + (rh - 28.0) / 2.0, 28.0, 28.0);
                    let emphasis = self.motion.hover(Action::Delete(id)).max(
                        if self.motion.pressed == Some(Action::Delete(id)) {
                            self.motion.pulse.value
                        } else {
                            0.0
                        },
                    );
                    let danger = color(0.78, 0.18, 0.23, 1.0);
                    p.fill(
                        delete,
                        9.0,
                        color(danger.r, danger.g, danger.b, 0.12 * emphasis),
                    );
                    let dc = color(
                        muted.r + (danger.r - muted.r) * emphasis,
                        muted.g + (danger.g - muted.g) * emphasis,
                        muted.b + (danger.b - muted.b) * emphasis,
                        0.72 + 0.28 * emphasis,
                    );
                    p.border(
                        rect(delete.x + 9.0, delete.y + 10.0, 10.0, 11.0),
                        1.6,
                        dc,
                        1.15,
                    );
                    p.line(
                        delete.x + 7.0,
                        delete.y + 8.0,
                        delete.x + 21.0,
                        delete.y + 8.0,
                        dc,
                        1.2,
                    );
                    p.line(
                        delete.x + 11.0,
                        delete.y + 5.5,
                        delete.x + 17.0,
                        delete.y + 5.5,
                        dc,
                        1.2,
                    );
                    p.line(
                        delete.x + 12.0,
                        delete.y + 13.0,
                        delete.x + 12.0,
                        delete.y + 18.0,
                        dc,
                        1.0,
                    );
                    p.line(
                        delete.x + 16.0,
                        delete.y + 13.0,
                        delete.x + 16.0,
                        delete.y + 18.0,
                        dc,
                        1.0,
                    );
                    if !row.retiring
                        && row.alpha.value > 0.3
                        && y >= top - 0.1
                        && y + rh <= bottom + 1.0
                        && fold < 0.1
                    {
                        self.hits.push(Hit {
                            r: rect(22.0, y, w - 44.0, rh),
                            action: Action::Row(id),
                        });
                        self.hits.push(Hit {
                            r: rect(22.0, y, 25.0, rh),
                            action: Action::Check(id),
                        });
                        self.hits.push(Hit {
                            r: status,
                            action: Action::Progress(id),
                        });
                        self.hits.push(Hit {
                            r: delete,
                            action: Action::Delete(id),
                        });
                    }
                    p.unclip();
                }
                self.motion.initialized = true;
                p.opacity(body_alpha);
                if active.is_empty() {
                    p.text(
                        "全部完成，歇一会儿",
                        rect(28.0, 88.0, w - 56.0, 35.0),
                        1,
                        muted,
                    );
                }
                if total > list_bottom - list_top {
                    let rail = list_bottom - list_top;
                    let thumb = (rail * rail / total).max(22.0);
                    let sy = list_top + self.motion.scroll.value / (total - rail) * (rail - thumb);
                    p.fill(
                        rect(w - 18.0, sy, 3.0, thumb),
                        1.5,
                        color(0.3, 0.4, 0.5, 0.35),
                    );
                }
                if self.dragging {
                    let yy = self.drag_y.clamp(list_top, list_bottom);
                    p.line(25.0, yy, w - 25.0, yy, accent, 2.0);
                }
                let add_feedback = self.motion.feedback(Action::Add);
                let inset = if self.motion.pressed == Some(Action::Add) {
                    self.motion.pulse.value * 1.4
                } else {
                    0.0
                };
                let ar = rect(
                    24.0 + inset,
                    add_y + inset,
                    w - 48.0 - 2.0 * inset,
                    40.0 - 2.0 * inset,
                );
                p.fill(ar, 14.0, color(1.0, 1.0, 1.0, 0.18 + add_feedback));
                p.border(ar, 14.0, color(0.3, 0.4, 0.5, 0.14 + add_feedback), 0.7);
                p.line(37.0, add_y + 14.0, 37.0, add_y + 25.0, muted, 1.2);
                p.line(31.5, add_y + 19.5, 42.5, add_y + 19.5, muted, 1.2);
                p.text(
                    "添加待办…",
                    rect(48.0, add_y + 10.0, w - 90.0, 24.0),
                    1,
                    muted,
                );
                p.fill(
                    rect(24.0, summary_y - 5.0, w - 48.0, 27.0),
                    10.0,
                    color(1.0, 1.0, 1.0, self.motion.feedback(Action::Completed)),
                );
                p.text(
                    &format!("已完成 {} 项", done.len()),
                    rect(
                        27.0,
                        summary_y,
                        w - if self.can_undo_delete { 195.0 } else { 88.0 },
                        22.0,
                    ),
                    0,
                    muted,
                );
                p.text(
                    if self.state.completed_open {
                        "收起"
                    } else {
                        "展开"
                    },
                    rect(w - 58.0, summary_y, 35.0, 22.0),
                    0,
                    muted,
                );
                if fold < 0.1 {
                    self.hits.push(Hit {
                        r: rect(24.0, add_y, w - 48.0, 40.0),
                        action: Action::Add,
                    });
                    self.hits.push(Hit {
                        r: rect(24.0, summary_y - 5.0, w - 48.0, 27.0),
                        action: Action::Completed,
                    });
                    if self.can_undo_delete {
                        let undo = rect(w - 145.0, summary_y - 5.0, 80.0, 27.0);
                        p.fill(
                            undo,
                            10.0,
                            color(
                                accent.r,
                                accent.g,
                                accent.b,
                                0.08 + self.motion.feedback(Action::UndoDelete),
                            ),
                        );
                        p.text(
                            "撤销删除",
                            rect(undo.x + 18.0, summary_y, 60.0, 22.0),
                            0,
                            accent,
                        );
                        self.hits.push(Hit {
                            r: undo,
                            action: Action::UndoDelete,
                        });
                    }
                }
                p.unclip();
                p.opacity(1.0);
            }
        }
    }
    fn action_at(&self, x: f32, y: f32) -> Option<Action> {
        self.hits
            .iter()
            .rev()
            .find(|h| h.r.has(x, y))
            .map(|h| h.action)
    }
    unsafe fn mouse_down(&mut self, p: POINT) {
        unsafe {
            let x = p.x as f32 / self.scale;
            let y = p.y as f32 / self.scale;
            self.motion.pressed = self.action_at(x, y);
            self.motion.pulse.snap(1.0);
            self.motion.pulse.set(0.0);
            self.schedule_motion();
            match self.action_at(x, y) {
                Some(Action::Delete(id)) => {
                    self.pending_delete = self
                        .todos
                        .iter()
                        .find(|t| t.id == id)
                        .map(|t| (id, t.version));
                    if self.pending_delete.is_some() && !self.hwnd.is_invalid() {
                        SetCapture(self.hwnd);
                    }
                    self.repaint();
                }
                Some(Action::UndoDelete) => {
                    let result = self.undo_delete();
                    self.report(result);
                }
                Some(Action::Settings) => self.open_settings(),
                Some(Action::Pin) => {
                    self.state.pinned = !self.state.pinned;
                    let _ = SetWindowPos(
                        self.hwnd,
                        Some(if self.state.pinned {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        }),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                    self.sync_backdrop();
                    self.save();
                    self.repaint();
                }
                Some(Action::Fold) => self.fold(),
                Some(Action::Add) => self.open_editor(None),
                Some(Action::Completed) => {
                    self.state.completed_open = !self.state.completed_open;
                    self.motion
                        .completed
                        .set(if self.state.completed_open { 1.0 } else { 0.0 });
                    self.save();
                    self.repaint();
                }
                Some(Action::Check(id)) => {
                    let result = self.core.get(id).and_then(|t| {
                        self.core.update(
                            id,
                            None,
                            Some(if t.status == Status::Done {
                                Status::Pending
                            } else {
                                Status::Done
                            }),
                            Some(t.version),
                        )
                    });
                    self.report(result.map(|_| ()));
                }
                Some(Action::Progress(id)) => {
                    let result = self.choose_status(id, p);
                    self.report(result);
                }
                Some(Action::Row(id)) => {
                    self.selected = Some(id);
                    self.pressed = self
                        .todos
                        .iter()
                        .find(|t| t.id == id)
                        .filter(|t| t.status != Status::Done)
                        .map(|_| (id, p));
                    self.dragging = false;
                    if self.pressed.is_some() {
                        SetCapture(self.hwnd);
                    }
                    self.repaint();
                }
                _ => {}
            }
        }
    }
    unsafe fn mouse_up(&mut self, p: POINT) {
        unsafe {
            if let Some((id, version)) = self.pending_delete.take() {
                let released_here = self
                    .action_at(p.x as f32 / self.scale, p.y as f32 / self.scale)
                    == Some(Action::Delete(id));
                if !self.hwnd.is_invalid() {
                    let _ = ReleaseCapture();
                }
                if released_here {
                    let result = self.delete_todo(id, Some(version));
                    self.report(result);
                }
                self.repaint();
                self.arm_dock_hide();
                return;
            }
            if let Some((id, _)) = self.pressed.take()
                && self.dragging
            {
                let rows: Vec<_> = self
                    .hits
                    .iter()
                    .filter_map(|h| {
                        if let Action::Row(i) = h.action
                            && self
                                .todos
                                .iter()
                                .any(|t| t.id == i && t.status != Status::Done)
                        {
                            Some((i, h.r.y + h.r.h / 2.0))
                        } else {
                            None
                        }
                    })
                    .collect();
                let mut ids: Vec<_> = self
                    .todos
                    .iter()
                    .filter(|t| t.status != Status::Done)
                    .map(|t| t.id)
                    .collect();
                let original = ids.iter().position(|i| *i == id).unwrap_or(0);
                ids.retain(|i| *i != id);
                let y = p.y as f32 / self.scale;
                let dest = drag_destination(&ids, &rows, id, y, original);
                ids.insert(dest, id);
                let result = self.core.reorder(&ids);
                self.report(result);
            }
            self.dragging = false;
            if !self.hwnd.is_invalid() {
                let _ = ReleaseCapture();
            }
            self.repaint();
            self.arm_dock_hide();
        }
    }
    unsafe fn fold(&mut self) {
        unsafe {
            self.detach_dock();
            if self.motion.fold_size.is_none() {
                self.save();
            }
            self.state.collapsed = !self.state.collapsed;
            self.motion.fold_size = Some((self.state.width as f32, self.state.height as f32));
            self.motion
                .fold
                .set(if self.state.collapsed { 1.0 } else { 0.0 });
            let _ = SetWindowPos(
                self.hwnd,
                None,
                0,
                0,
                (self.state.width.max(288) as f32 * self.scale) as i32,
                (self.state.height as f32 * self.scale) as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let _ = self.core.save_window(&self.state);
            self.repaint();
        }
    }
    unsafe fn menu(&mut self, point: POINT) {
        unsafe {
            self.set_dock_open(true, false);
            let result = (|| -> Result<()> {
                let mut local = point;
                let _ = ScreenToClient(self.hwnd, &mut local);
                let id = match self
                    .action_at(local.x as f32 / self.scale, local.y as f32 / self.scale)
                {
                    Some(
                        Action::Row(id)
                        | Action::Check(id)
                        | Action::Progress(id)
                        | Action::Delete(id),
                    ) => Some(id),
                    _ => self.selected,
                };
                let m = CreatePopupMenu()?;
                if id.is_some() {
                    AppendMenuW(m, MF_STRING, 1, w!("编辑\tF2"))?;
                    AppendMenuW(m, MF_STRING, 2, w!("标记为进行中"))?;
                    AppendMenuW(m, MF_STRING, 3, w!("标记为未完成"))?;
                    AppendMenuW(m, MF_STRING, 4, w!("标记为已完成"))?;
                    AppendMenuW(m, MF_STRING, 5, w!("删除\tDelete"))?;
                    AppendMenuW(m, MF_SEPARATOR, 0, None)?;
                }
                AppendMenuW(m, MF_STRING, 6, w!("新增待办\tCtrl+N"))?;
                AppendMenuW(m, MF_STRING, 7, w!("撤销最近删除\tCtrl+Z"))?;
                AppendMenuW(m, MF_STRING, 13, w!("设置…\tCtrl+,"))?;
                AppendMenuW(m, MF_SEPARATOR, 0, None)?;
                AppendMenuW(
                    m,
                    MF_STRING
                        | if self.state.autostart {
                            MF_CHECKED
                        } else {
                            MF_UNCHECKED
                        },
                    8,
                    w!("登录 Windows 后自动启动"),
                )?;
                AppendMenuW(m, MF_STRING, 9, w!("打开 MCP 连接配置"))?;
                AppendMenuW(m, MF_STRING, 10, w!("打开数据目录"))?;
                AppendMenuW(m, MF_SEPARATOR, 0, None)?;
                AppendMenuW(m, MF_STRING, 11, w!("隐藏到托盘"))?;
                AppendMenuW(m, MF_STRING, 12, w!("退出"))?;
                let _ = SetForegroundWindow(self.hwnd);
                let cmd = TrackPopupMenu(
                    m,
                    TPM_RETURNCMD | TPM_RIGHTBUTTON,
                    point.x,
                    point.y,
                    None,
                    self.hwnd,
                    None,
                )
                .0;
                DestroyMenu(m)?;
                match cmd {
                    1 => {
                        if let Some(id) = id {
                            self.open_editor(Some(id));
                        }
                    }
                    2..=4 => {
                        if let Some(id) = id {
                            self.core.update(
                                id,
                                None,
                                Some(match cmd {
                                    2 => Status::InProgress,
                                    3 => Status::Pending,
                                    _ => Status::Done,
                                }),
                                None,
                            )?;
                        }
                    }
                    5 => {
                        if let Some(id) = id {
                            self.delete_todo(id, None)?;
                        }
                    }
                    6 => self.open_editor(None),
                    7 => {
                        self.undo_delete()?;
                    }
                    8 => {
                        let enabled = !self.state.autostart;
                        set_autostart(enabled, &self.cfg)?;
                        self.state.autostart = enabled;
                        self.save();
                    }
                    9 => open_path(&self.cfg.directory.join("mcp-configs")),
                    10 => open_path(&self.cfg.directory),
                    11 => {
                        send(self.hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
                    }
                    12 => self.quit(),
                    13 => self.open_settings(),
                    _ => {}
                }
                Ok(())
            })();
            self.report(result);
            self.arm_dock_hide();
        }
    }
    unsafe fn choose_status(&mut self, id: i64, point: POINT) -> Result<()> {
        unsafe {
            if IsWindow(Some(self.status_picker)).as_bool() {
                let _ = DestroyWindow(self.status_picker);
            }
            let mut window = RECT::default();
            GetWindowRect(self.hwnd, &mut window)?;
            let anchor = if let Some(hit) = self
                .hits
                .iter()
                .find(|hit| hit.action == Action::Progress(id))
            {
                RECT {
                    left: window.left + (hit.r.x * self.scale).round() as i32,
                    top: window.top + (hit.r.y * self.scale).round() as i32,
                    right: window.left + ((hit.r.x + hit.r.w) * self.scale).round() as i32,
                    bottom: window.top + ((hit.r.y + hit.r.h) * self.scale).round() as i32,
                }
            } else {
                let mut point = point;
                let _ = ClientToScreen(self.hwnd, &mut point);
                RECT {
                    left: point.x,
                    top: point.y,
                    right: point.x,
                    bottom: point.y,
                }
            };
            self.status_picker = crate::status_picker::open(
                self.hwnd,
                self.core.clone(),
                id,
                anchor,
                self.scale,
                self.dark,
            )?;
            Ok(())
        }
    }
    unsafe fn tray(&self, add: bool) {
        unsafe {
            let mut data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: 1,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: WM_TRAY,
                hIcon: LoadIconW(
                    Some(GetModuleHandleW(None).unwrap_or_default().into()),
                    PCWSTR(std::ptr::without_provenance(1)),
                )
                .unwrap_or_default(),
                ..Default::default()
            };
            let tip = wide("玻璃待办 · 双击/点击显示，右键菜单");
            for (i, c) in tip.into_iter().take(data.szTip.len()).enumerate() {
                data.szTip[i] = c;
            }
            let _ = Shell_NotifyIconW(if add { NIM_ADD } else { NIM_DELETE }, &data);
        }
    }
    unsafe fn quit(&mut self) {
        unsafe {
            self.save();
            self.tray(false);
            if IsWindow(Some(self.editor)).as_bool() {
                let _ = DestroyWindow(self.editor);
            }
            if IsWindow(Some(self.settings)).as_bool() {
                let _ = DestroyWindow(self.settings);
            }
            if IsWindow(Some(self.status_picker)).as_bool() {
                let _ = DestroyWindow(self.status_picker);
            }
            if !self.backdrop.is_invalid() {
                let _ = DestroyWindow(self.backdrop);
                self.backdrop = HWND::default();
            }
            let _ = DestroyWindow(self.hwnd);
        }
    }
    unsafe fn open_editor(&mut self, id: Option<i64>) {
        unsafe {
            self.set_dock_open(true, false);
            if self.state.collapsed {
                self.fold();
            }
            if IsWindow(Some(self.editor)).as_bool() {
                let _ = SetForegroundWindow(self.editor);
                return;
            }
            match Editor::open(
                self.hwnd,
                self.core.clone(),
                id,
                self.scale,
                self.dark,
                self.appearance.text_size,
            ) {
                Ok(hwnd) => self.editor = hwnd,
                Err(e) => self.report(Err(e)),
            }
        }
    }
    unsafe fn open_settings(&mut self) {
        unsafe {
            self.set_dock_open(true, false);
            if IsWindow(Some(self.settings)).as_bool() {
                let _ = SetForegroundWindow(self.settings);
                return;
            }
            match crate::preferences::open(self.hwnd, self.core.clone(), self.scale) {
                Ok(hwnd) => self.settings = hwnd,
                Err(e) => self.report(Err(e)),
            }
        }
    }
    unsafe fn create_backdrop(&mut self, instance: HINSTANCE) {
        unsafe {
            // A separate, input-transparent DWM surface sits under the alpha-painted note.
            // It is only retained when the documented Win11 backdrop attribute succeeds.
            let class = w!("GlassTodo.Backdrop");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(backdrop_proc),
                hInstance: instance,
                lpszClassName: class,
                hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
                ..Default::default()
            };
            RegisterClassW(&wc);
            if let Ok(bg) = CreateWindowExW(
                WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE
                    | if self.state.pinned {
                        WS_EX_TOPMOST
                    } else {
                        WINDOW_EX_STYLE(0)
                    },
                class,
                w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(instance),
                None,
            ) {
                let material = 3u32;
                let supported = DwmSetWindowAttribute(
                    bg,
                    DWMWINDOWATTRIBUTE(38),
                    &material as *const _ as *const _,
                    4,
                )
                .is_ok();
                if supported {
                    let margins = windows::Win32::UI::Controls::MARGINS {
                        cxLeftWidth: -1,
                        cxRightWidth: -1,
                        cyTopHeight: -1,
                        cyBottomHeight: -1,
                    };
                    let _ = DwmExtendFrameIntoClientArea(bg, &margins);
                    let corner = 2u32;
                    let _ = DwmSetWindowAttribute(
                        bg,
                        DWMWINDOWATTRIBUTE(33),
                        &corner as *const _ as *const _,
                        4,
                    );
                    self.backdrop = bg;
                    SetWindowLongPtrW(self.hwnd, GWLP_HWNDPARENT, bg.0 as isize);
                    self.sync_backdrop();
                } else {
                    let _ = DestroyWindow(bg);
                }
            }
        }
    }
    unsafe fn sync_backdrop(&self) {
        unsafe {
            if self.backdrop.is_invalid() {
                return;
            }
            if self.dock.as_ref().is_some_and(|dock| !dock.interactive()) {
                let _ = ShowWindow(self.backdrop, SW_HIDE);
                return;
            }
            let mut r = RECT::default();
            if GetWindowRect(self.hwnd, &mut r).is_ok() {
                let inset = (8.0 * self.scale) as i32;
                if let Some((w, h)) = self.motion.fold_size {
                    let f = self.motion.fold.value;
                    r.right = r.left + ((w + (288.0 - w) * f) * self.scale) as i32;
                    r.bottom = r.top + ((h + (86.0 - h) * f) * self.scale) as i32;
                }
                let _ = SetWindowPos(
                    self.backdrop,
                    Some(if self.state.pinned {
                        HWND_TOPMOST
                    } else {
                        HWND_NOTOPMOST
                    }),
                    r.left + inset,
                    r.top + inset,
                    r.right - r.left - 2 * inset,
                    r.bottom - r.top - 2 * inset,
                    SWP_NOACTIVATE,
                );
                if IsWindowVisible(self.hwnd).as_bool() {
                    let _ = ShowWindow(self.backdrop, SW_SHOWNOACTIVATE);
                }
            }
        }
    }
}

unsafe extern "system" fn backdrop_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
            WM_ERASEBKGND => LRESULT(1),
            _ => DefWindowProcW(hwnd, msg, w, l),
        }
    }
}
unsafe fn work_area() -> RECT {
    unsafe {
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let _ = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            Some((&mut r as *mut RECT).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        );
        r
    }
}
pub(crate) unsafe fn is_dark() -> bool {
    unsafe {
        let mut value = 1u32;
        let mut size = 4;
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        )
        .is_ok()
            && value == 0
    }
}
unsafe fn open_path(path: &Path) {
    unsafe {
        let p = wide(&path.to_string_lossy());
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(p.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        );
    }
}

fn set_autostart(enabled: bool, cfg: &Config) -> Result<()> {
    unsafe {
        let mut key = HKEY::default();
        RegCreateKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_SET_VALUE,
            None,
            &mut key,
            None,
        )
        .ok()?;
        let result = if enabled {
            let exe = std::env::current_exe()?;
            let command = wide(&format!(
                "\"{}\" --data-dir \"{}\" --port {}",
                exe.display(),
                cfg.directory.display(),
                cfg.port
            ));
            let bytes =
                std::slice::from_raw_parts(command.as_ptr().cast::<u8>(), command.len() * 2);
            RegSetValueExW(key, w!("GlassTodo"), None, REG_SZ, Some(bytes)).ok()
        } else {
            let r = RegDeleteValueW(key, w!("GlassTodo"));
            if r == ERROR_FILE_NOT_FOUND {
                Ok(())
            } else {
                r.ok()
            }
        };
        let _ = RegCloseKey(key);
        result?;
        Ok(())
    }
}

const EDITOR_DESTROY: u32 = WM_APP + 21;
const EDITOR_HOVER: u32 = WM_APP + 22;
struct Editor {
    attached: bool,
    owns_owner: bool,
    owner: HWND,
    hwnd: HWND,
    edit: HWND,
    core: Arc<Core>,
    id: Option<i64>,
    version: Option<i64>,
    font: HFONT,
    brush: HBRUSH,
    composing: bool,
    focused: bool,
    scale: f32,
    dark: bool,
    painter: Painter,
    width: f32,
    height: f32,
    count: usize,
    wheel_remainder: i32,
    error: String,
    reveal: Spring,
    primary_hover: Spring,
    cancel_hover: Spring,
    closing: bool,
    saving: bool,
    last: Instant,
    timer: bool,
    x: i32,
    y: i32,
    enabled: bool,
}
struct EditorInitialization(HWND);
impl Drop for EditorInitialization {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_invalid() {
                let _ = DestroyWindow(self.0);
            }
        }
    }
}
impl Drop for Editor {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(HGDIOBJ(self.font.0));
            let _ = DeleteObject(HGDIOBJ(self.brush.0));
        }
    }
}
fn editor_surface(dark: bool) -> COLORREF {
    if dark {
        COLORREF(35 | (47 << 8) | (63 << 16))
    } else {
        COLORREF(230 | (237 << 8) | (245 << 16))
    }
}
pub(crate) fn load_native_input() -> Result<()> {
    // System DLL, loaded only when an input panel is first opened. Keep it
    // loaded while registered control classes and TSF callbacks may reference it.
    static MODULE: std::sync::OnceLock<std::result::Result<usize, String>> =
        std::sync::OnceLock::new();
    let module = MODULE.get_or_init(|| unsafe {
        LoadLibraryExW(w!("Msftedit.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32)
            .map(|m| m.0 as usize)
            .map_err(|e| e.to_string())
    });
    match module {
        Ok(_) => Ok(()),
        Err(e) => anyhow::bail!("加载系统输入组件失败: {e}"),
    }
}
pub(crate) unsafe fn configure_native_input(hwnd: HWND, text_size: u8, dark: bool) -> Result<()> {
    unsafe {
        anyhow::ensure!(
            send(
                hwnd,
                EM_SETTEXTMODE,
                WPARAM((TM_PLAINTEXT.0 | TM_MULTICODEPAGE.0 | TM_MULTILEVELUNDO.0) as usize),
                LPARAM(0)
            )
            .0 == 0,
            "无法启用纯文本输入"
        );
        apply_native_input_style(hwnd, text_size, dark)?;
        send(
            hwnd,
            EM_SETEVENTMASK,
            WPARAM(0),
            LPARAM(ENM_CHANGE as isize),
        );
        send(hwnd, EM_EXLIMITTEXT, WPARAM(0), LPARAM(262144));
        Ok(())
    }
}
pub(crate) unsafe fn apply_native_input_style(hwnd: HWND, text_size: u8, dark: bool) -> Result<()> {
    unsafe {
        send(
            hwnd,
            EM_SETBKGNDCOLOR,
            WPARAM(0),
            LPARAM(editor_surface(dark).0 as isize),
        );
        let mut format = CHARFORMAT2W::default();
        format.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
        format.Base.dwMask = CFM_COLOR | CFM_BACKCOLOR | CFM_FACE | CFM_SIZE;
        format.Base.crTextColor = if dark {
            COLORREF(0x00fff1e8)
        } else {
            COLORREF(0x00402d19)
        };
        format.Base.yHeight = text_size as i32 * 15; // DIP -> twentieths of a point.
        let face = wide("Segoe UI");
        format.Base.szFaceName[..face.len()].copy_from_slice(&face);
        format.crBackColor = editor_surface(dark);
        // Plain-text mode has one document-wide format; selection formatting
        // is intentionally unsupported by the native control in this mode.
        anyhow::ensure!(
            send(
                hwnd,
                EM_SETCHARFORMAT,
                WPARAM((SCF_DEFAULT | SCF_NOKBUPDATE) as usize),
                LPARAM((&format as *const CHARFORMAT2W) as isize)
            )
            .0 != 0,
            "无法设置输入文字样式"
        );
        Ok(())
    }
}
unsafe fn draw_composer(p: &Painter, w: f32, h: f32, dark: bool, editing: bool, error: &str) {
    unsafe {
        let bg = if dark {
            color(27.0 / 255.0, 36.0 / 255.0, 48.0 / 255.0, 1.0)
        } else {
            color(241.0 / 255.0, 246.0 / 255.0, 252.0 / 255.0, 1.0)
        };
        let field = if dark {
            color(35.0 / 255.0, 47.0 / 255.0, 63.0 / 255.0, 1.0)
        } else {
            color(230.0 / 255.0, 237.0 / 255.0, 245.0 / 255.0, 1.0)
        };
        let ink = if dark {
            color(0.92, 0.95, 1.0, 1.0)
        } else {
            color(0.10, 0.17, 0.25, 1.0)
        };
        let muted = if dark {
            color(0.64, 0.72, 0.83, 1.0)
        } else {
            color(0.40, 0.48, 0.57, 1.0)
        };
        p.fill(rect(0.0, 0.0, w, h), 23.0, bg);
        p.border(
            rect(0.8, 0.8, w - 1.6, h - 1.6),
            22.0,
            color(1.0, 1.0, 1.0, if dark { 0.18 } else { 0.95 }),
            1.4,
        );
        p.circle(32.0, 34.0, 14.0, color(0.29, 0.57, 0.85, 0.12), true);
        p.line(27.0, 34.0, 37.0, 34.0, color(0.23, 0.50, 0.76, 1.0), 1.5);
        p.line(32.0, 29.0, 32.0, 39.0, color(0.23, 0.50, 0.76, 1.0), 1.5);
        p.text(
            if editing {
                "编辑待办"
            } else {
                "添加待办"
            },
            rect(54.0, 22.0, w - 72.0, 28.0),
            2,
            ink,
        );
        p.text(
            if editing {
                "保留原有状态，支持多行正文"
            } else {
                "空一行分隔待办，支持多行正文"
            },
            rect(20.0, 58.0, w - 40.0, 20.0),
            0,
            muted,
        );
        p.fill(rect(18.0, 84.0, w - 36.0, h - 190.0), 17.0, field);
        p.border(
            rect(18.0, 84.0, w - 36.0, h - 190.0),
            17.0,
            color(0.30, 0.53, 0.76, 0.19),
            1.0,
        );
        if error.is_empty() {
            p.text(
                if editing {
                    "Enter 保存 · Shift+Enter 换行 · Esc 取消"
                } else {
                    "Shift+Enter 换行 · Enter 添加 · Esc 取消"
                },
                rect(20.0, h - 94.0, w - 40.0, 28.0),
                0,
                muted,
            );
        } else {
            p.text(
                error,
                rect(20.0, h - 94.0, w - 40.0, 30.0),
                0,
                color(0.77, 0.24, 0.25, 1.0),
            );
        }
    }
}
impl Editor {
    unsafe fn open(
        owner: HWND,
        core: Arc<Core>,
        id: Option<i64>,
        scale: f32,
        dark: bool,
        text_size: u8,
    ) -> Result<HWND> {
        unsafe { Self::create(owner, core, id, scale, dark, text_size, true) }
    }
    unsafe fn create(
        owner: HWND,
        core: Arc<Core>,
        id: Option<i64>,
        scale: f32,
        dark: bool,
        text_size: u8,
        show: bool,
    ) -> Result<HWND> {
        unsafe {
            load_native_input()?;
            let todo = id.map(|id| core.get(id)).transpose()?;
            let mut painter = Painter::new()?;
            painter.set_text_size(text_size)?;
            let instance = GetModuleHandleW(None)?;
            let class = w!("GlassTodo.Editor");
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(editor_proc),
                hInstance: instance.into(),
                lpszClassName: class,
                hCursor: LoadCursorW(None, IDC_ARROW)?,
                ..Default::default()
            });
            let font = CreateFontW(
                -(text_size as f32 * scale) as i32,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                CLEARTYPE_QUALITY,
                0,
                w!("Segoe UI"),
            );
            let mut r = RECT::default();
            GetWindowRect(owner, &mut r)?;
            let width = ((r.right - r.left) as f32 / scale - 24.0).max(256.0);
            let height = ((r.bottom - r.top) as f32 / scale - 24.0).max(300.0);
            let x = r.left + (12.0 * scale) as i32;
            let y = r.top + (12.0 * scale) as i32;
            let enabled = animations_enabled();
            let ptr = Box::into_raw(Box::new(Editor {
                attached: false,
                owns_owner: false,
                owner,
                hwnd: HWND::default(),
                edit: HWND::default(),
                core,
                id,
                version: todo.as_ref().map(|t| t.version),
                font,
                brush: CreateSolidBrush(editor_surface(dark)),
                composing: false,
                focused: false,
                scale,
                dark,
                painter,
                width,
                height,
                count: 0,
                wheel_remainder: 0,
                error: String::new(),
                reveal: Spring::new(if enabled { 0.0 } else { 1.0 }),
                primary_hover: Spring::new(0.0),
                cancel_hover: Spring::new(0.0),
                closing: false,
                saving: false,
                last: Instant::now(),
                timer: false,
                x,
                y,
                enabled,
            }));
            let ww = (width * scale) as i32;
            let hh = (height * scale) as i32;
            let hwnd = match CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_LAYERED,
                class,
                if id.is_some() {
                    w!("编辑待办")
                } else {
                    w!("添加待办")
                },
                WS_POPUP | WS_CLIPCHILDREN,
                x,
                y,
                ww,
                hh,
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
            let mut initialization = EditorInitialization(hwnd);
            let region = CreateRoundRectRgn(
                0,
                0,
                ww + 1,
                hh + 1,
                (46.0 * scale) as i32,
                (46.0 * scale) as i32,
            );
            SetWindowRgn(hwnd, Some(region), false);
            let text = wide(
                &todo
                    .as_ref()
                    .map(|t| t.content.replace('\n', "\r\n"))
                    .unwrap_or_default(),
            );
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                MSFTEDIT_CLASS,
                w!(""),
                WS_CHILD
                    | WS_VISIBLE
                    | WS_TABSTOP
                    | WS_VSCROLL
                    | WINDOW_STYLE(
                        ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | ES_WANTRETURN as u32,
                    ),
                (30.0 * scale) as i32,
                (98.0 * scale) as i32,
                ((width - 60.0) * scale) as i32,
                ((height - 218.0) * scale) as i32,
                Some(hwnd),
                Some(HMENU(EDIT_ID as *mut _)),
                Some(instance.into()),
                None,
            )?;
            (*ptr).edit = edit;
            send(edit, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(1));
            configure_native_input(edit, text_size, dark)?;
            anyhow::ensure!(
                SetWindowSubclass(edit, Some(edit_subclass), 1, ptr as usize).as_bool(),
                "无法初始化输入交互"
            );
            SetWindowTextW(edit, PCWSTR(text.as_ptr()))?;
            for (bid, label, bx, bw) in [
                (102, w!("取消"), 18.0, 66.0),
                (101, w!("添加"), width - 144.0, 126.0),
            ] {
                let button = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("BUTTON"),
                    label,
                    WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_OWNERDRAW as u32),
                    (bx * scale) as i32,
                    ((height - 58.0) * scale) as i32,
                    (bw * scale) as i32,
                    (36.0 * scale) as i32,
                    Some(hwnd),
                    Some(HMENU(bid as *mut _)),
                    Some(instance.into()),
                    None,
                )?;
                send(button, WM_SETFONT, WPARAM(font.0 as usize), LPARAM(0));
                let _ = SetWindowSubclass(button, Some(button_subclass), 2, ptr as usize);
            }
            (*ptr).changed();
            (*ptr).reveal.set(1.0);
            SetLayeredWindowAttributes(
                hwnd,
                COLORREF(0),
                if enabled { 0 } else { 255 },
                LWA_ALPHA,
            )?;
            if show {
                (*ptr).owns_owner = true;
                let _ = EnableWindow(owner, false);
                let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
                let _ = SetForegroundWindow(hwnd);
                let _ = SetFocus(Some(edit));
                send(edit, EM_SETSEL, WPARAM(0), LPARAM(-1));
                (*ptr).schedule();
            }
            initialization.0 = HWND::default();
            Ok(hwnd)
        }
    }
    unsafe fn wheel(&mut self, w: WPARAM) -> bool {
        unsafe {
            // Leave Ctrl+wheel to Rich Edit. Process ordinary wheel messages
            // explicitly so scrolling also works without activating the input.
            if w.0 & 0x0008 != 0 {
                return false;
            }
            self.wheel_remainder += (w.0 >> 16) as u16 as i16 as i32;
            let steps = self.wheel_remainder / 120;
            self.wheel_remainder %= 120;
            if steps == 0 {
                return true;
            }
            let mut lines = 3u32;
            let _ = SystemParametersInfoW(
                SPI_GETWHEELSCROLLLINES,
                0,
                Some((&mut lines as *mut u32).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
            if lines == u32::MAX {
                for _ in 0..steps.unsigned_abs() {
                    send(
                        self.edit,
                        WM_VSCROLL,
                        WPARAM(if steps > 0 {
                            SB_PAGEUP.0
                        } else {
                            SB_PAGEDOWN.0
                        } as usize),
                        LPARAM(0),
                    );
                }
            } else if lines != 0 {
                let offset =
                    (-(steps as i64) * lines as i64).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                send(self.edit, EM_LINESCROLL, WPARAM(0), LPARAM(offset as isize));
            }
            true
        }
    }
    unsafe fn value(&self) -> String {
        unsafe {
            let len = GetWindowTextLengthW(self.edit).max(0) as usize;
            let mut text = vec![0u16; len + 1];
            let n = GetWindowTextW(self.edit, &mut text);
            String::from_utf16_lossy(&text[..n as usize])
                .replace("\r\n", "\n")
                .replace(['\r', '\u{b}'], "\n")
        }
    }
    unsafe fn changed(&mut self) {
        unsafe {
            if !self.edit.is_invalid() {
                let _ = InvalidateRect(Some(self.edit), None, true);
            }
            let value = self.value();
            self.count = if self.id.is_some() {
                usize::from(!value.trim().is_empty())
            } else {
                split_todo_blocks(&value).len()
            };
            self.error.clear();
            if let Ok(button) = GetDlgItem(Some(self.hwnd), 101) {
                let label = wide(&if self.id.is_some() {
                    "保存更改".into()
                } else if self.count == 0 {
                    "添加待办".into()
                } else {
                    format!("添加 {} 条", self.count)
                });
                let _ = SetWindowTextW(button, PCWSTR(label.as_ptr()));
                let _ = EnableWindow(button, self.count > 0);
            }
            let _ = InvalidateRect(Some(self.hwnd), None, false);
        }
    }
    unsafe fn save(&mut self) {
        unsafe {
            if self.closing || self.saving || self.composing {
                return;
            }
            let value = self.value();
            let result = if let Some(id) = self.id {
                self.core
                    .update(id, Some(&value), None, self.version)
                    .map(|_| ())
            } else {
                self.core.create_batch(&value).map(|_| ())
            };
            match result {
                Ok(()) => {
                    self.saving = true;
                    self.close();
                }
                Err(e) => {
                    self.error = e.to_string();
                    let _ = InvalidateRect(Some(self.hwnd), None, false);
                }
            }
        }
    }
    unsafe fn close(&mut self) {
        unsafe {
            if self.closing {
                return;
            }
            self.closing = true;
            self.reveal.set(0.0);
            self.schedule();
        }
    }
    unsafe fn schedule(&mut self) {
        unsafe {
            if !self.timer {
                self.timer = true;
                self.last = Instant::now();
                SetTimer(Some(self.hwnd), MOTION_TIMER, 16, None);
            }
        }
    }
    unsafe fn animate(&mut self) {
        unsafe {
            let now = Instant::now();
            let dt = if self.enabled {
                now.duration_since(self.last).as_secs_f32()
            } else {
                5.0
            };
            self.last = now;
            for s in [
                &mut self.reveal,
                &mut self.primary_hover,
                &mut self.cancel_hover,
            ] {
                s.step(dt);
            }
            let v = self.reveal.value;
            let _ = SetLayeredWindowAttributes(
                self.hwnd,
                COLORREF(0),
                (v.clamp(0.0, 1.0) * 255.0) as u8,
                LWA_ALPHA,
            );
            let _ = SetWindowPos(
                self.hwnd,
                None,
                self.x,
                self.y + ((1.0 - v) * 10.0 * self.scale) as i32,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            for id in [101, 102] {
                if let Ok(h) = GetDlgItem(Some(self.hwnd), id) {
                    let _ = InvalidateRect(Some(h), None, false);
                }
            }
            if !self.reveal.active() && !self.primary_hover.active() && !self.cancel_hover.active()
            {
                let _ = KillTimer(Some(self.hwnd), MOTION_TIMER);
                self.timer = false;
                if self.closing {
                    let _ = PostMessageW(Some(self.hwnd), EDITOR_DESTROY, WPARAM(0), LPARAM(0));
                }
            }
        }
    }
    unsafe fn paint(&mut self, dc: HDC) -> Result<()> {
        unsafe {
            self.painter.begin(
                (self.width * self.scale) as i32,
                (self.height * self.scale) as i32,
                self.scale,
            )?;
            draw_composer(
                &self.painter,
                self.width,
                self.height,
                self.dark,
                self.id.is_some(),
                &self.error,
            );
            self.painter.present(dc)
        }
    }
    unsafe fn button(&mut self, item: &DRAWITEMSTRUCT) -> Result<()> {
        unsafe {
            let w = (item.rcItem.right - item.rcItem.left) as f32 / self.scale;
            let h = (item.rcItem.bottom - item.rcItem.top) as f32 / self.scale;
            self.painter
                .begin(item.rcItem.right, item.rcItem.bottom, self.scale)?;
            let p = &self.painter;
            let bg = if self.dark {
                color(27.0 / 255.0, 36.0 / 255.0, 48.0 / 255.0, 1.0)
            } else {
                color(241.0 / 255.0, 246.0 / 255.0, 252.0 / 255.0, 1.0)
            };
            p.fill(rect(0.0, 0.0, w, h), 0.0, bg);
            let primary = item.CtlID == 101;
            let disabled = item.itemState.0 & ODS_DISABLED.0 != 0;
            let hover = if primary {
                self.primary_hover.value
            } else {
                self.cancel_hover.value
            };
            let inset = if item.itemState.0 & ODS_SELECTED.0 != 0 {
                1.2
            } else {
                0.0
            };
            let bc = if primary && !disabled {
                color(
                    0.18 + hover * 0.04,
                    0.45 + hover * 0.04,
                    0.73 + hover * 0.04,
                    1.0,
                )
            } else if self.dark {
                color(
                    0.18 + hover * 0.04,
                    0.23 + hover * 0.04,
                    0.30 + hover * 0.04,
                    1.0,
                )
            } else {
                color(
                    0.88 - hover * 0.04,
                    0.92 - hover * 0.03,
                    0.96 - hover * 0.02,
                    1.0,
                )
            };
            p.fill(
                rect(inset, inset, w - 2.0 * inset, h - 2.0 * inset),
                12.0,
                bc,
            );
            let ink = if disabled {
                color(0.51, 0.59, 0.66, 1.0)
            } else if primary || self.dark {
                color(1.0, 1.0, 1.0, 1.0)
            } else {
                color(0.23, 0.32, 0.43, 1.0)
            };
            let label = if primary {
                if self.id.is_some() {
                    "保存更改".into()
                } else if self.count > 0 {
                    format!("添加 {} 条", self.count)
                } else {
                    "添加待办".into()
                }
            } else {
                "取消".into()
            };
            p.text(&label, rect(8.0, 9.0, w - 16.0, 23.0), 3, ink);
            if item.itemState.0 & ODS_FOCUS.0 != 0 {
                p.border(
                    rect(2.0, 2.0, w - 4.0, h - 4.0),
                    10.0,
                    color(0.55, 0.76, 0.98, 0.9),
                    1.2,
                );
            }
            p.present(item.hDC)
        }
    }
}
unsafe extern "system" fn editor_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        if msg == WM_NCCREATE {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Editor;
        if ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        match msg {
            WM_SAVE_EDIT => {
                (*ptr).save();
                LRESULT(0)
            }
            WM_CLOSE => {
                (*ptr).close();
                LRESULT(0)
            }
            EDITOR_DESTROY => {
                let _ = DestroyWindow(hwnd);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == MOTION_TIMER => {
                (*ptr).animate();
                LRESULT(0)
            }
            WM_COMMAND => {
                let id = wparam.0 & 0xffff;
                let code = (wparam.0 >> 16) & 0xffff;
                if id == EDIT_ID && code == EN_CHANGE as usize {
                    (*ptr).changed();
                } else if id == 101 && code == BN_CLICKED as usize {
                    (*ptr).save();
                } else if id == 102 && code == BN_CLICKED as usize {
                    (*ptr).close();
                }
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                if (*ptr).wheel(wparam) {
                    LRESULT(0)
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            EDITOR_HOVER => {
                let target = if lparam.0 != 0 { 1.0 } else { 0.0 };
                if wparam.0 == 101 {
                    (*ptr).primary_hover.set(target);
                } else {
                    (*ptr).cancel_hover.set(target);
                }
                (*ptr).schedule();
                LRESULT(0)
            }
            WM_CTLCOLOREDIT => {
                let dc = HDC(wparam.0 as *mut _);
                SetBkColor(dc, editor_surface((*ptr).dark));
                SetTextColor(
                    dc,
                    if (*ptr).dark {
                        COLORREF(0x00fff1e8)
                    } else {
                        COLORREF(0x00402d19)
                    },
                );
                LRESULT((*ptr).brush.0 as isize)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_DRAWITEM => {
                let _ = (*ptr).button(&*(lparam.0 as *const DRAWITEMSTRUCT));
                LRESULT(1)
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut ps);
                let _ = (*ptr).paint(dc);
                let _ = EndPaint(hwnd, &ps);
                LRESULT(0)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                if !(*ptr).attached {
                    return DefWindowProcW(hwnd, msg, wparam, lparam);
                }
                let editor = Box::from_raw(ptr);
                let _ = KillTimer(Some(hwnd), MOTION_TIMER);
                if editor.owns_owner {
                    let _ = EnableWindow(editor.owner, true);
                    if IsWindowVisible(editor.owner).as_bool() {
                        let _ = SetForegroundWindow(editor.owner);
                    }
                }
                let _ = PostMessageW(Some(editor.owner), WM_REFRESH, WPARAM(0), LPARAM(0));
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
unsafe extern "system" fn button_subclass(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let ptr = data as *mut Editor;
        if msg == WM_MOUSEMOVE || msg == WM_MOUSELEAVE {
            let _ = PostMessageW(
                Some((*ptr).hwnd),
                EDITOR_HOVER,
                WPARAM(GetDlgCtrlID(hwnd) as usize),
                LPARAM(if msg == WM_MOUSEMOVE { 1 } else { 0 }),
            );
            if msg == WM_MOUSEMOVE {
                let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                });
            }
        }
        if msg == WM_KEYDOWN && w.0 as u16 == VK_ESCAPE.0 {
            let _ = PostMessageW(Some((*ptr).hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
            return LRESULT(0);
        }
        if msg == WM_KEYDOWN && w.0 as u16 == VK_TAB.0 {
            let id = GetDlgCtrlID(hwnd);
            let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
            let next = if id == 101 && !shift {
                GetDlgItem(Some((*ptr).hwnd), 102)
            } else if id == 102 && shift {
                GetDlgItem(Some((*ptr).hwnd), 101)
            } else {
                Ok((*ptr).edit)
            };
            if let Ok(h) = next {
                let _ = SetFocus(Some(h));
            }
            return LRESULT(0);
        }
        if msg == WM_KEYDOWN && w.0 as u16 == VK_RETURN.0 {
            let _ = PostMessageW(
                Some((*ptr).hwnd),
                WM_COMMAND,
                WPARAM(GetDlgCtrlID(hwnd) as usize),
                LPARAM(hwnd.0 as isize),
            );
            return LRESULT(0);
        }
        DefSubclassProc(hwnd, msg, w, l)
    }
}
unsafe extern "system" fn edit_subclass(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    unsafe {
        let editor = data as *mut Editor;
        match msg {
            WM_MOUSEWHEEL if (*editor).wheel(w) => return LRESULT(0),
            WM_SETFOCUS => {
                (*editor).focused = true;
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            WM_KILLFOCUS => {
                (*editor).focused = false;
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            WM_IME_STARTCOMPOSITION => {
                (*editor).composing = true;
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            WM_IME_ENDCOMPOSITION => {
                (*editor).composing = false;
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            WM_KEYDOWN if w.0 as u16 == VK_RETURN.0 => {
                let context = ImmGetContext(hwnd);
                let composing = (*editor).composing
                    || (!context.is_invalid()
                        && ImmGetCompositionStringW(context, GCS_COMPSTR, None, 0) > 0);
                if !context.is_invalid() {
                    let _ = ImmReleaseContext(hwnd, context);
                }
                if !composing && GetKeyState(VK_SHIFT.0 as i32) >= 0 {
                    PostMessageW(Some((*editor).hwnd), WM_SAVE_EDIT, WPARAM(0), LPARAM(0)).ok();
                    return LRESULT(0);
                }
                if !composing {
                    let newline = wide("\r\n");
                    send(
                        hwnd,
                        EM_REPLACESEL,
                        WPARAM(1),
                        LPARAM(newline.as_ptr() as isize),
                    );
                    return LRESULT(0);
                }
            }
            WM_CHAR if w.0 as u16 == VK_RETURN.0 && !(*editor).composing => {
                // TranslateMessage may already have queued WM_CHAR before the
                // save shortcut. Do not insert an extra newline at the caret.
                return LRESULT(0);
            }
            WM_KEYDOWN if w.0 as u16 == VK_ESCAPE.0 && !(*editor).composing => {
                PostMessageW(Some((*editor).hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)).ok();
                return LRESULT(0);
            }
            WM_KEYDOWN if w.0 as u16 == VK_TAB.0 && !(*editor).composing => {
                let bid = if GetKeyState(VK_SHIFT.0 as i32) < 0 {
                    102
                } else {
                    101
                };
                if let Ok(button) = GetDlgItem(Some((*editor).hwnd), bid) {
                    if IsWindowEnabled(button).as_bool() {
                        let _ = SetFocus(Some(button));
                    } else if let Ok(cancel) = GetDlgItem(Some((*editor).hwnd), 102) {
                        let _ = SetFocus(Some(cancel));
                    }
                }
                return LRESULT(0);
            }
            WM_CHAR if w.0 as u16 == VK_TAB.0 => return LRESULT(0),
            _ => {}
        }
        let result = DefSubclassProc(hwnd, msg, w, l);
        if [
            WM_SETFOCUS,
            WM_KILLFOCUS,
            WM_IME_STARTCOMPOSITION,
            WM_IME_ENDCOMPOSITION,
            WM_CHAR,
            WM_SETTEXT,
            WM_PASTE,
            WM_CUT,
            WM_CLEAR,
            WM_UNDO,
            EM_REPLACESEL,
        ]
        .contains(&msg)
        {
            let _ = InvalidateRect(Some(hwnd), None, true);
        }
        if (msg == WM_PAINT || msg == WM_PRINTCLIENT)
            && GetWindowTextLengthW(hwnd) == 0
            && !(*editor).composing
            && !(*editor).focused
        {
            let dc = if msg == WM_PRINTCLIENT {
                HDC(w.0 as *mut _)
            } else {
                GetDC(Some(hwnd))
            };
            SelectObject(dc, HGDIOBJ((*editor).font.0));
            SetBkMode(dc, TRANSPARENT);
            SetTextColor(
                dc,
                if (*editor).dark {
                    COLORREF(0x00bda58e)
                } else {
                    COLORREF(0x009b8874)
                },
            );
            let mut r = RECT::default();
            send(
                hwnd,
                EM_GETRECT,
                WPARAM(0),
                LPARAM((&mut r as *mut RECT) as isize),
            );
            let mut text = wide(if (*editor).id.is_some() {
                "写下待办内容…"
            } else {
                "想到什么，就记在这里…\n空一行，添加下一条待办"
            });
            text.pop();
            DrawTextW(dc, &mut text, &mut r, DT_LEFT | DT_WORDBREAK);
            if msg != WM_PRINTCLIENT {
                ReleaseDC(Some(hwnd), dc);
            }
        }
        result
    }
}

unsafe fn send(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    unsafe { SendMessageW(hwnd, msg, Some(w), Some(l)) }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct NativeNoteFixture {
        app: Box<Ui>,
        class: Vec<u16>,
        instance: HINSTANCE,
    }
    impl NativeNoteFixture {
        unsafe fn new() -> Self {
            unsafe {
                static NEXT: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let serial = NEXT.fetch_add(1, Ordering::Relaxed);
                let directory = std::path::PathBuf::from(".scratch/qa-native-dock");
                std::fs::create_dir_all(&directory).unwrap();
                let core = Arc::new(
                    Core::new(&directory.join(format!("{}-{serial}.sqlite3", std::process::id())))
                        .unwrap(),
                );
                let mut app = Box::new(note_fixture(core, directory));
                let instance = GetModuleHandleW(None).unwrap().into();
                let class = wide(&format!("GlassTodo.QaDock.{}.{serial}", std::process::id()));
                assert_ne!(
                    RegisterClassW(&WNDCLASSW {
                        lpfnWndProc: Some(window_proc),
                        hInstance: instance,
                        lpszClassName: PCWSTR(class.as_ptr()),
                        ..Default::default()
                    }),
                    0
                );
                let pointer = &mut *app as *mut Ui;
                app.hwnd = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    PCWSTR(class.as_ptr()),
                    w!("Private hidden test"),
                    WS_POPUP | WS_THICKFRAME,
                    -32000,
                    -32000,
                    474,
                    549,
                    None,
                    None,
                    Some(instance),
                    Some(pointer.cast()),
                )
                .unwrap();
                let work = window_work_area(app.hwnd);
                app.place_bounds(RECT {
                    left: work.left + 100,
                    top: work.top + 100,
                    right: work.left + 100 + (316.0 * app.scale).round() as i32,
                    bottom: work.top + 100 + (366.0 * app.scale).round() as i32,
                });
                assert!(!IsWindowVisible(app.hwnd).as_bool());
                Self {
                    app,
                    class,
                    instance,
                }
            }
        }
        unsafe fn size(&self) -> (i32, i32) {
            unsafe {
                let mut bounds = RECT::default();
                GetWindowRect(self.app.hwnd, &mut bounds).unwrap();
                (bounds.right - bounds.left, bounds.bottom - bounds.top)
            }
        }
    }
    impl Drop for NativeNoteFixture {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.app.hwnd);
                let _ = UnregisterClassW(PCWSTR(self.class.as_ptr()), Some(self.instance));
            }
        }
    }
    #[test]
    fn docking_release_does_not_hover_expand_from_the_old_panel() {
        unsafe {
            for nonclient in [false, true] {
                let mut fixture = NativeNoteFixture::new();
                let work = window_work_area(fixture.app.hwnd);
                let (width, height) = fixture.size();
                let bounds = RECT {
                    left: work.left - 24,
                    top: work.top + 100,
                    right: work.left - 24 + width,
                    bottom: work.top + 100 + height,
                };
                fixture.app.place_bounds(bounds);
                fixture.app.motion.enabled = true;
                fixture.app.native_drag = true;
                fixture.app.moving_window = true;
                let point = POINT {
                    x: bounds.left + width / 2,
                    y: bounds.top + 20,
                };
                fixture.app.end_window_drag_at(Some(point));
                let dock = fixture.app.dock.as_ref().unwrap();
                assert!(!dock.open && dock.active());
                assert!(
                    point.y < dock.layout.icon.top,
                    "the mouse remains over the old panel, outside the destination icon"
                );
                fixture.app.dock_pointer_move(nonclient, Some(point));
                let dock = fixture.app.dock.as_ref().unwrap();
                assert!(
                    !dock.open,
                    "a mouse-move notification from the shrinking panel must not reverse docking into hover expansion"
                );
                assert_eq!(dock.reveal.target, 0.0);
                let icon = dock.layout.icon;
                for _ in 0..150 {
                    fixture.app.motion.last = Instant::now() - std::time::Duration::from_millis(16);
                    fixture.app.animate();
                }
                fixture.app.dock_pointer_move(
                    nonclient,
                    Some(POINT {
                        x: (icon.left + icon.right) / 2,
                        y: (icon.top + icon.bottom) / 2,
                    }),
                );
                assert!(
                    fixture.app.dock.as_ref().unwrap().open,
                    "a release outside the icon should allow the first later icon entry"
                );
            }
        }
    }
    #[test]
    fn native_fresh_dock_ignores_stationary_hover_until_real_exit_and_reentry() {
        unsafe {
            let mut fixture = NativeNoteFixture::new();
            let work = window_work_area(fixture.app.hwnd);
            let (width, height) = fixture.size();
            fixture.app.place_bounds(RECT {
                left: work.left - 24,
                top: work.top + 100,
                right: work.left - 24 + width,
                bottom: work.top + 100 + height,
            });
            fixture.app.motion.enabled = true;
            fixture.app.native_drag = true;
            fixture.app.moving_window = true;
            let mut bounds = RECT::default();
            GetWindowRect(fixture.app.hwnd, &mut bounds).unwrap();
            let anchor = docking::detect(bounds, work, fixture.app.scale).unwrap();
            let icon = Layout::new(
                anchor,
                fixture.app.state.width,
                fixture.app.state.height,
                fixture.app.scale,
                work,
            )
            .icon;
            let point = POINT {
                x: (icon.left + icon.right) / 2,
                y: (icon.top + icon.bottom) / 2,
            };
            fixture.app.end_window_drag_at(Some(point));
            fixture.app.dock_pointer_move(false, Some(point));
            assert!(!fixture.app.dock.as_ref().unwrap().open);
            for _ in 0..150 {
                fixture.app.motion.last = Instant::now() - std::time::Duration::from_millis(16);
                fixture.app.animate();
            }
            assert_eq!(
                fixture.size(),
                (icon.right - icon.left, icon.bottom - icon.top)
            );
            fixture.app.dock_pointer_leave(Some(point));
            fixture.app.dock_pointer_move(true, Some(point));
            assert!(
                !fixture.app.dock.as_ref().unwrap().open,
                "client/nonclient leave notifications over the icon do not count as an exit"
            );
            let outside = POINT {
                x: icon.right + 100,
                y: icon.bottom + 100,
            };
            fixture.app.dock_pointer_leave(Some(outside));
            fixture.app.dock_pointer_move(false, Some(outside));
            assert!(!fixture.app.dock.as_ref().unwrap().open);
            fixture.app.dock_pointer_move(false, Some(point));
            assert!(fixture.app.dock.as_ref().unwrap().open);
            assert_eq!(fixture.app.dock.as_ref().unwrap().reveal.target, 1.0);
            assert!(!IsWindowVisible(fixture.app.hwnd).as_bool());
        }
    }
    #[test]
    fn native_dock_leave_inside_the_transparent_margin_still_queues_a_hide_check() {
        unsafe {
            let mut fixture = NativeNoteFixture::new();
            let work = RECT {
                left: -32000,
                top: -32000,
                right: -30000,
                bottom: -30000,
            };
            let anchor = crate::store::DockAnchor {
                edge: crate::store::DockEdge::Left,
                position: 0.4,
            };
            fixture.app.state.dock = Some(anchor);
            fixture.app.dock = Some(Dock::new(Layout::new(
                anchor,
                316,
                366,
                fixture.app.scale,
                work,
            )));
            fixture.app.set_dock_open(true, false);
            let bounds = fixture.app.dock.as_ref().unwrap().bounds();
            // The painted side starts at 8 DIP. A real leave can arrive while
            // the pointer is still inside the native bounding rectangle.
            let point = POINT {
                x: bounds.left + 1,
                y: (bounds.top + bounds.bottom) / 2,
            };
            assert!(docking::contains(bounds, point));
            fixture.app.cancel_dock_hide();
            fixture.app.dock_pointer_leave(Some(point));
            std::thread::sleep(std::time::Duration::from_millis(
                u64::from(docking::HIDE_DELAY_MS) + 80,
            ));
            let mut message = MSG::default();
            let mut queued = false;
            while PeekMessageW(
                &mut message,
                Some(fixture.app.hwnd),
                WM_TIMER,
                WM_TIMER,
                PM_REMOVE,
            )
            .as_bool()
            {
                queued |= message.wParam.0 == DOCK_HIDE_TIMER;
            }
            assert!(
                queued,
                "a leave inside transparent padding must still schedule the delayed hide check"
            );
            fixture.app.check_dock_hide();
            assert!(
                !fixture.app.dock.as_ref().unwrap().open,
                "a pointer that keeps moving outside must collapse without another move message"
            );
            let icon = fixture.app.dock.as_ref().unwrap().layout.icon;
            assert_eq!(
                fixture.size(),
                (icon.right - icon.left, icon.bottom - icon.top)
            );
            assert!(fixture.app.dock_hide_due.is_none());
        }
    }
    #[test]
    fn native_dock_refreshes_keep_the_first_leave_deadline_and_stale_timers_cannot_close_early() {
        unsafe {
            let mut fixture = NativeNoteFixture::new();
            let work = RECT {
                left: -32000,
                top: -32000,
                right: -30000,
                bottom: -30000,
            };
            let anchor = crate::store::DockAnchor {
                edge: crate::store::DockEdge::Right,
                position: 0.4,
            };
            fixture.app.state.dock = Some(anchor);
            fixture.app.dock = Some(Dock::new(Layout::new(
                anchor,
                316,
                366,
                fixture.app.scale,
                work,
            )));
            fixture.app.set_dock_open(true, false);
            let first = fixture.app.dock_hide_due.unwrap();
            for _ in 0..3 {
                std::thread::sleep(std::time::Duration::from_millis(90));
                fixture.app.refresh();
                assert_eq!(
                    fixture.app.dock_hide_due,
                    Some(first),
                    "agent refreshes must not postpone the initial leave deadline"
                );
            }
            std::thread::sleep(std::time::Duration::from_millis(90));
            let mut message = MSG::default();
            let mut queued = false;
            while PeekMessageW(
                &mut message,
                Some(fixture.app.hwnd),
                WM_TIMER,
                WM_TIMER,
                PM_REMOVE,
            )
            .as_bool()
            {
                queued |= message.wParam.0 == DOCK_HIDE_TIMER;
            }
            assert!(
                queued,
                "the native timer must expire even while data updates keep arriving"
            );
            fixture.app.cancel_dock_hide();
            fixture.app.dock_hide_timeout(Instant::now());
            assert!(
                fixture.app.dock.as_ref().unwrap().open,
                "a canceled timer callback must have no effect"
            );
            fixture.app.start_dock_hide();
            let due = fixture.app.dock_hide_due.unwrap();
            fixture
                .app
                .dock_hide_timeout(due - std::time::Duration::from_millis(20));
            assert!(fixture.app.dock.as_ref().unwrap().open);
            assert_eq!(fixture.app.dock_hide_due, Some(due));
            fixture.app.dock_hide_timeout(due);
            assert!(!fixture.app.dock.as_ref().unwrap().open);
            assert!(fixture.app.dock_hide_due.is_none());
            assert!(
                !fixture.app.motion.timer,
                "a settled icon must not keep a polling or animation timer running"
            );
        }
    }
    #[test]
    fn dock_leave_tracking_distinguishes_client_content_and_native_header() {
        unsafe {
            let mut fixture = NativeNoteFixture::new();
            let mut bounds = RECT::default();
            GetWindowRect(fixture.app.hwnd, &mut bounds).unwrap();
            let scale = fixture.app.scale;
            let body = POINT {
                x: bounds.left + (150.0 * scale) as i32,
                y: bounds.top + (200.0 * scale) as i32,
            };
            let header = POINT {
                x: bounds.left + (150.0 * scale) as i32,
                y: bounds.top + (40.0 * scale) as i32,
            };
            assert_eq!(fixture.app.dock_tracking_flags(body), TME_LEAVE);
            assert_eq!(
                fixture.app.dock_tracking_flags(header),
                TME_LEAVE | TME_NONCLIENT
            );
            fixture.app.arm_dock_hide();
            assert!(
                fixture.app.dock_hide_due.is_none(),
                "regular notes do not start a hide timer"
            );
        }
    }
    #[test]
    fn native_dock_preserves_size_animates_to_rest_and_keeps_owned_editor_open() {
        unsafe {
            let mut fixture = NativeNoteFixture::new();
            let scale = fixture.app.scale;
            let work = window_work_area(fixture.app.hwnd);
            let (width, height) = fixture.size();
            fixture.app.place_bounds(RECT {
                left: work.left - 24,
                top: work.top + 100,
                right: work.left - 24 + width,
                bottom: work.top + 100 + height,
            });
            fixture.app.native_drag = true;
            fixture.app.moving_window = true;
            fixture.app.end_window_drag();
            fixture.app.save();
            let icon_size = (docking::ICON_SIZE * scale).round() as i32;
            assert_eq!(
                fixture.size(),
                (icon_size, icon_size),
                "real WS_THICKFRAME window must shrink to the icon size"
            );
            assert_eq!(
                fixture.app.core.window().dock.unwrap().edge,
                crate::store::DockEdge::Left
            );
            assert_eq!(
                (fixture.app.state.width, fixture.app.state.height),
                (316, 366)
            );
            fixture.app.set_dock_open(true, false);
            assert_eq!(fixture.size(), (width, height));
            assert!(fixture.app.hits.iter().any(|h| h.action == Action::Add));
            fixture.app.save();
            assert_eq!(
                (
                    fixture.app.core.window().width,
                    fixture.app.core.window().height
                ),
                (316, 366)
            );
            fixture.app.motion.enabled = true;
            fixture.app.set_dock_open(false, true);
            for _ in 0..100 {
                fixture.app.motion.last = Instant::now() - std::time::Duration::from_millis(16);
                fixture.app.animate();
            }
            assert_eq!(fixture.size(), (icon_size, icon_size));
            assert!(
                !fixture.app.motion.timer,
                "docking animation must stop its timer when settled"
            );
            fixture.app.set_dock_open(true, true);
            for _ in 0..100 {
                fixture.app.motion.last = Instant::now() - std::time::Duration::from_millis(16);
                fixture.app.animate();
            }
            assert_eq!(fixture.size(), (width, height));
            assert!(!fixture.app.motion.timer);
            fixture.app.motion.enabled = false;
            fixture.app.place_bounds(RECT {
                left: -32000,
                top: -32000,
                right: -32000 + width,
                bottom: -32000 + height,
            });
            fixture.app.editor = Editor::create(
                fixture.app.hwnd,
                fixture.app.core.clone(),
                None,
                scale,
                false,
                14,
                false,
            )
            .unwrap();
            fixture.app.check_dock_hide();
            assert!(
                fixture.app.dock.as_ref().unwrap().open,
                "an open input panel must keep the note expanded"
            );
            DestroyWindow(fixture.app.editor).unwrap();
            fixture.app.editor = HWND::default();
            fixture.app.check_dock_hide();
            assert!(!fixture.app.dock.as_ref().unwrap().open);
            fixture.app.set_dock_open(true, false);
            fixture.app.place_bounds(RECT {
                left: -32000,
                top: -32000,
                right: -32000 + width,
                bottom: -32000 + height,
            });
            fixture.app.settings = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("STATIC"),
                w!("Private settings lifetime fixture"),
                WS_POPUP,
                -32000,
                -32000,
                100,
                100,
                Some(fixture.app.hwnd),
                None,
                Some(fixture.instance),
                None,
            )
            .unwrap();
            fixture.app.check_dock_hide();
            assert!(
                fixture.app.dock.as_ref().unwrap().open,
                "an open settings panel must keep the note expanded"
            );
            DestroyWindow(fixture.app.settings).unwrap();
            fixture.app.settings = HWND::default();
            fixture.app.check_dock_hide();
            assert!(!fixture.app.dock.as_ref().unwrap().open);
            fixture.app.set_dock_open(true, false);
            fixture.app.begin_window_drag(true);
            assert!(fixture.app.state.dock.is_none());
            assert!(fixture.app.dock.is_none());
            let x = work.left + ((work.right - work.left) - width) / 2;
            let y = work.top + ((work.bottom - work.top) - height) / 2;
            fixture.app.place_bounds(RECT {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            });
            fixture.app.end_window_drag();
            fixture.app.save();
            assert!(
                fixture.app.core.window().dock.is_none(),
                "dragging away from an edge must restore a regular note"
            );
            assert_eq!(fixture.size(), (width, height));
            assert!(!IsWindowVisible(fixture.app.hwnd).as_bool());
        }
    }
    #[test]
    fn dock_icon_render_and_hover_content_expose_only_visible_actions() {
        unsafe {
            let directory = std::path::PathBuf::from(".scratch/qa-dock-render");
            std::fs::create_dir_all(&directory).unwrap();
            let core = Arc::new(
                Core::new(&directory.join(format!("{}.sqlite3", std::process::id()))).unwrap(),
            );
            core.create("贴边后的完整待办内容").unwrap();
            let mut app = note_fixture(core, directory.clone());
            let work = RECT {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1040,
            };
            for dark in [false, true] {
                app.dark = dark;
                app.dock = Some(Dock::new(Layout::new(
                    crate::store::DockAnchor {
                        edge: crate::store::DockEdge::Right,
                        position: 0.5,
                    },
                    316,
                    366,
                    1.5,
                    work,
                )));
                app.painter.begin(72, 72, 1.5).unwrap();
                app.draw(48.0, 48.0);
                let pixels = app.painter.test_pixels().unwrap();
                assert!(pixels[(36 * 72 + 36) * 4 + 3] > 240);
                assert!(app.hits.is_empty());
                assert_eq!(app.hit_test(24.0, 24.0, 48.0, 48.0), HTCLIENT);
                std::fs::write(
                    directory.join(if dark {
                        "dock-dark-72x72.bgra"
                    } else {
                        "dock-light-72x72.bgra"
                    }),
                    pixels,
                )
                .unwrap();
                let dock = app.dock.as_mut().unwrap();
                dock.set_open(true, false);
                app.painter.begin(474, 549, 1.5).unwrap();
                app.draw(316.0, 366.0);
                app.painter.test_pixels().unwrap();
                assert!(app.hits.iter().any(|hit| hit.action == Action::Add));
                assert!(
                    app.hits
                        .iter()
                        .any(|hit| matches!(hit.action, Action::Row(_)))
                );
                app.dock.as_mut().unwrap().set_open(false, true);
                app.dock.as_mut().unwrap().reveal.step(0.016);
                app.painter.begin(474, 549, 1.5).unwrap();
                app.draw(316.0, 366.0);
                app.painter.test_pixels().unwrap();
                assert!(
                    app.hits.is_empty(),
                    "items cannot receive clicks during the shrink transition"
                );
            }
        }
    }
    #[test]
    fn resizing_limits_follow_dpi_and_allow_the_fold_animation_canvas() {
        let work = RECT {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1040,
        };
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let (minimum, maximum) = track_limits(scale, false, false, work);
            assert_eq!(
                (minimum.x, minimum.y),
                (
                    (316.0 * scale).round() as i32,
                    (366.0 * scale).round() as i32
                )
            );
            assert_eq!((maximum.x, maximum.y), (1920, 1040));
            let (minimum, maximum) = track_limits(scale, true, false, work);
            assert_eq!(
                (minimum.x, minimum.y),
                (
                    (288.0 * scale).round() as i32,
                    (86.0 * scale).round() as i32
                )
            );
            assert_eq!((maximum.x, maximum.y), (minimum.x, minimum.y));
            for collapsed in [false, true] {
                let (_, maximum) = track_limits(scale, collapsed, true, work);
                assert_eq!(
                    (maximum.x, maximum.y),
                    (1920, 1040),
                    "fold transition needs its full canvas"
                );
            }
        }
        let (minimum, maximum) = track_limits(
            2.0,
            false,
            false,
            RECT {
                right: 200,
                bottom: 200,
                ..Default::default()
            },
        );
        assert_eq!((maximum.x, maximum.y), (minimum.x, minimum.y));
    }
    #[test]
    fn resizing_supports_four_corners_without_taking_header_controls() {
        unsafe {
            let directory = std::path::PathBuf::from(".scratch/qa-resize-hits");
            std::fs::create_dir_all(&directory).unwrap();
            let core = Arc::new(
                Core::new(&directory.join(format!("{}.sqlite3", std::process::id()))).unwrap(),
            );
            let mut app = note_fixture(core, directory);
            app.painter.begin(474, 549, 1.5).unwrap();
            app.draw(316.0, 366.0);
            app.painter.test_pixels().unwrap();
            for (x, y, expected) in [
                (1.0, 1.0, HTTOPLEFT),
                (16.0, 16.0, HTTOPLEFT),
                (315.0, 1.0, HTTOPRIGHT),
                (300.0, 16.0, HTTOPRIGHT),
                (1.0, 365.0, HTBOTTOMLEFT),
                (16.0, 350.0, HTBOTTOMLEFT),
                (315.0, 365.0, HTBOTTOMRIGHT),
                (300.0, 350.0, HTBOTTOMRIGHT),
                (16.0, 180.0, HTLEFT),
                (300.0, 180.0, HTRIGHT),
                (158.0, 16.0, HTTOP),
                (158.0, 350.0, HTBOTTOM),
            ] {
                assert_eq!(
                    app.hit_test(x, y, 316.0, 366.0),
                    expected,
                    "corner at {x},{y}"
                );
            }
            assert_eq!(app.hit_test(27.0, 32.0, 316.0, 366.0), HTCAPTION);
            for action in [Action::Settings, Action::Pin, Action::Fold] {
                let point = action_point(&app, action);
                assert_eq!(
                    app.hit_test(
                        point.x as f32 / app.scale,
                        point.y as f32 / app.scale,
                        316.0,
                        366.0
                    ),
                    HTCLIENT
                );
            }
            app.state.collapsed = true;
            assert_eq!(app.hit_test(1.0, 1.0, 288.0, 86.0), HTCLIENT);
            app.state.collapsed = false;
            app.motion.fold_size = Some((316.0, 366.0));
            assert_eq!(app.hit_test(1.0, 1.0, 316.0, 366.0), HTCLIENT);
        }
    }
    #[test]
    fn resizing_round_corners_receive_mouse_hits() {
        unsafe {
            let directory = std::path::PathBuf::from(".scratch/qa-resize-pixels");
            std::fs::create_dir_all(&directory).unwrap();
            let core = Arc::new(
                Core::new(&directory.join(format!("{}.sqlite3", std::process::id()))).unwrap(),
            );
            let mut app = note_fixture(core, directory);
            app.painter.begin(474, 549, 1.5).unwrap();
            app.draw(316.0, 366.0);
            let pixels = app.painter.test_pixels().unwrap();
            for (x, y) in [(0, 0), (473, 0), (0, 548), (473, 548)] {
                assert!(
                    pixels[(y * 474 + x) * 4 + 3] > 0,
                    "corner at {x},{y} must receive layered-window mouse hits"
                );
            }
        }
    }
    unsafe fn note_fixture(core: Arc<Core>, directory: std::path::PathBuf) -> Ui {
        unsafe {
            let cfg = Config {
                directory: directory.clone(),
                port: 57841,
                no_autostart: true,
                headless: true,
                exit_after: None,
            };
            Ui {
                hwnd: HWND::default(),
                core: core.clone(),
                cfg,
                state: WindowState::default(),
                appearance: Appearance::default(),
                settings: HWND::default(),
                status_picker: HWND::default(),
                painter: Painter::new().unwrap(),
                todos: core.list().unwrap(),
                hits: vec![],
                scale: 1.5,
                scroll: 0.0,
                selected: None,
                pending_delete: None,
                can_undo_delete: core.can_undo_delete().unwrap(),
                reveal_todo: None,
                pressed: None,
                dragging: false,
                drag_y: 0.0,
                editor: HWND::default(),
                backdrop: HWND::default(),
                dark: false,
                done_scroll: 0.0,
                completed_area: rect(0.0, 0.0, 0.0, 0.0),
                dispatching: false,
                deferred_refresh: false,
                motion: Motion::new(&WindowState::default(), false),
                dock: None,
                dock_hide_due: None,
                native_drag: false,
                moving_window: false,
            }
        }
    }
    unsafe fn render_note(app: &mut Ui) {
        unsafe {
            app.painter.begin(474, 720, 1.5).unwrap();
            app.draw(316.0, 480.0);
            app.painter.test_pixels().unwrap();
        }
    }
    fn action_point(app: &Ui, action: Action) -> POINT {
        let r = app
            .hits
            .iter()
            .find(|h| h.action == action)
            .expect("action must be visible")
            .r;
        POINT {
            x: ((r.x + r.w / 2.0) * app.scale) as i32,
            y: ((r.y + r.h / 2.0) * app.scale) as i32,
        }
    }
    #[test]
    fn delete_button_release_cancel_and_undo_restore_original_items() {
        unsafe {
            let dir = std::path::PathBuf::from(".scratch/qa-delete-flow");
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join(format!("{}.sqlite3", std::process::id()));
            let core = Arc::new(Core::new(&path).unwrap());
            let pending = core.create("可以删除的事项").unwrap();
            let progress = core.create("正在处理的事项").unwrap();
            core.update(progress.id, None, Some(Status::InProgress), None)
                .unwrap();
            let done = core.create("已完成的事项").unwrap();
            core.update(done.id, None, Some(Status::Done), None)
                .unwrap();
            let mut app = note_fixture(core.clone(), dir);
            app.state.completed_open = true;
            app.motion.completed.snap(1.0);
            render_note(&mut app);
            for id in [pending.id, progress.id, done.id] {
                let point = action_point(&app, Action::Delete(id));
                assert!(
                    app.action_at(point.x as f32 / app.scale, point.y as f32 / app.scale)
                        == Some(Action::Delete(id))
                );
            }
            let delete = app
                .hits
                .iter()
                .find(|h| h.action == Action::Delete(progress.id))
                .unwrap()
                .r;
            let status = app
                .hits
                .iter()
                .find(|h| h.action == Action::Progress(progress.id))
                .unwrap()
                .r;
            assert!(
                status.x + status.w < delete.x,
                "delete must not overlap the progress action"
            );
            let point = action_point(&app, Action::Delete(pending.id));
            app.mouse_down(point);
            assert!(
                core.get(pending.id).is_ok(),
                "pressing alone must not delete"
            );
            app.mouse_up(POINT { x: 0, y: 0 });
            assert!(
                core.get(pending.id).is_ok(),
                "dragging outside must cancel deletion"
            );
            app.mouse_down(point);
            app.mouse_up(point);
            assert!(core.get(pending.id).is_err());
            app.refresh();
            render_note(&mut app);
            assert!(app.can_undo_delete);
            assert!(
                !app.hits
                    .iter()
                    .any(|h| h.action == Action::Delete(pending.id))
            );
            let undo = action_point(&app, Action::UndoDelete);
            app.mouse_down(undo);
            app.refresh();
            render_note(&mut app);
            let restored = core.get(pending.id).unwrap();
            assert_eq!(restored.content, pending.content);
            assert_eq!(restored.status, pending.status);
            assert_eq!(restored.position, pending.position);
            assert!(!app.can_undo_delete);
            let point = action_point(&app, Action::Delete(done.id));
            app.mouse_down(point);
            app.mouse_up(point);
            app.refresh();
            render_note(&mut app);
            let undo = action_point(&app, Action::UndoDelete);
            app.mouse_down(undo);
            app.refresh();
            render_note(&mut app);
            assert_eq!(core.get(done.id).unwrap().status, Status::Done);
            let original = core.get(progress.id).unwrap();
            app.delete_todo(progress.id, None).unwrap();
            app.refresh();
            assert!(app.can_undo_delete);
            drop(app);
            drop(core);
            let reopened = Core::new(&path).unwrap();
            assert!(reopened.can_undo_delete().unwrap());
            let restored = reopened.undo().unwrap();
            assert_eq!(restored.id, original.id);
            assert_eq!(restored.status, Status::InProgress);
            assert_eq!(restored.position, original.position);
            assert_eq!(restored.version, original.version + 2);
            assert!(!reopened.can_undo_delete().unwrap());
        }
    }
    struct InputFixture {
        owner: HWND,
        dialog: HWND,
        edit: HWND,
        dc: HDC,
        bitmap: HBITMAP,
        old: HGDIOBJ,
        pixels: *mut u8,
        width: i32,
        height: i32,
    }
    impl InputFixture {
        unsafe fn new(dark: bool) -> Self {
            unsafe { Self::with_content(dark, None) }
        }
        unsafe fn with_content(dark: bool, content: Option<&str>) -> Self {
            unsafe {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let dir = std::path::PathBuf::from(".scratch/qa-input-regressions");
                std::fs::create_dir_all(&dir).unwrap();
                let core = Arc::new(
                    Core::new(&dir.join(format!(
                        "{}-{}.sqlite3",
                        std::process::id(),
                        NEXT.fetch_add(1, Ordering::Relaxed)
                    )))
                    .unwrap(),
                );
                let todo = content.map(|content| core.create(content).unwrap());
                let owner = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    w!("STATIC"),
                    w!("hidden input regression"),
                    WS_POPUP,
                    -32000,
                    -32000,
                    316,
                    366,
                    None,
                    None,
                    None,
                    None,
                )
                .unwrap();
                let dialog =
                    Editor::create(owner, core, todo.map(|todo| todo.id), 1.0, dark, 14, false)
                        .unwrap();
                let _ = ShowWindow(dialog, SW_SHOWNOACTIVATE);
                let ptr = GetWindowLongPtrW(dialog, GWLP_USERDATA) as *mut Editor;
                let edit = (*ptr).edit;
                let mut r = RECT::default();
                GetClientRect(edit, &mut r).unwrap();
                let dc = CreateCompatibleDC(None);
                let mut pixels = std::ptr::null_mut();
                let info = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: r.right,
                        biHeight: -r.bottom,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let bitmap =
                    CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut pixels, None, 0)
                        .unwrap();
                let old = SelectObject(dc, HGDIOBJ(bitmap.0));
                FillRect(dc, &r, (*ptr).brush);

                Self {
                    owner,
                    dialog,
                    edit,
                    dc,
                    bitmap,
                    old,
                    pixels: pixels.cast(),
                    width: r.right,
                    height: r.bottom,
                }
            }
        }
        unsafe fn print(&self) {
            unsafe {
                send(
                    self.edit,
                    WM_PRINTCLIENT,
                    WPARAM(self.dc.0 as usize),
                    LPARAM((PRF_CLIENT | PRF_ERASEBKGND) as isize),
                );
                let _ = GdiFlush();
            }
        }
        unsafe fn non_background_pixels(&self, dark: bool) -> usize {
            unsafe {
                let c = editor_surface(dark).0;
                let expected = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
                std::slice::from_raw_parts(self.pixels, (self.width * self.height * 4) as usize)
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[..3] != expected)
                    .count()
            }
        }
    }
    impl Drop for InputFixture {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.dialog);
                let _ = DestroyWindow(self.owner);
                SelectObject(self.dc, self.old);
                let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
                let _ = DeleteDC(self.dc);
            }
        }
    }
    #[test]
    fn input_regression_long_content_scrolls_with_wheel_and_scrollbar_without_changing_text() {
        unsafe {
            let content = (1..=120)
                .map(|line| format!("第 {line} 行：多行待办正文，滚动时保持内容与状态。\n"))
                .collect::<String>();
            for editing in [true, false] {
                let f = InputFixture::with_content(false, editing.then_some(content.as_str()));
                let ptr = GetWindowLongPtrW(f.dialog, GWLP_USERDATA) as *mut Editor;
                if !editing {
                    let text = wide(&content);
                    SetWindowTextW(f.edit, PCWSTR(text.as_ptr())).unwrap();
                }
                let value = (*ptr).value();
                send(f.edit, EM_SETSEL, WPARAM(0), LPARAM(0));
                send(f.edit, EM_SCROLLCARET, WPARAM(0), LPARAM(0));
                assert_eq!(
                    send(f.edit, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0,
                    0
                );
                let mut point = POINT { x: 12, y: 12 };
                let _ = ClientToScreen(f.edit, &mut point);
                let position =
                    LPARAM(((point.y as u16 as u32) << 16 | point.x as u16 as u32) as isize);
                send(
                    f.edit,
                    WM_MOUSEWHEEL,
                    WPARAM(((-120i16) as u16 as usize) << 16),
                    position,
                );
                let down = send(f.edit, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0;
                assert!(
                    down > 0,
                    "mouse wheel must move long content down; editing={editing}, first line={down}"
                );
                send(f.edit, WM_MOUSEWHEEL, WPARAM((120usize) << 16), position);
                let up = send(f.edit, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0;
                assert!(up < down, "mouse wheel must also move back up");
                send(f.edit, WM_VSCROLL, WPARAM(SB_TOP.0 as usize), LPARAM(0));
                send(
                    f.dialog,
                    WM_MOUSEWHEEL,
                    WPARAM(((-60i16) as u16 as usize) << 16),
                    position,
                );
                assert_eq!(
                    send(f.edit, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0,
                    0,
                    "partial high-resolution wheel deltas must accumulate"
                );
                send(
                    f.dialog,
                    WM_MOUSEWHEEL,
                    WPARAM(((-60i16) as u16 as usize) << 16),
                    position,
                );
                assert!(
                    send(f.edit, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0 > 0,
                    "the panel must also route wheel input to its text viewport"
                );
                assert!(
                    !(*ptr).wheel(WPARAM(0x0008 | (((-120i16) as u16 as usize) << 16))),
                    "Ctrl+wheel is left to the native input"
                );
                let mut info = SCROLLINFO {
                    cbSize: std::mem::size_of::<SCROLLINFO>() as u32,
                    fMask: SIF_ALL,
                    ..Default::default()
                };
                GetScrollInfo(f.edit, SB_VERT, &mut info).unwrap();
                assert!(
                    info.nMax - info.nMin + 1 > info.nPage as i32,
                    "overflow must expose a scrollable vertical range"
                );
                assert_ne!(
                    GetWindowLongW(f.edit, GWL_STYLE) as u32 & WS_VSCROLL.0,
                    0,
                    "long text needs a vertical scrollbar"
                );
                send(f.edit, WM_VSCROLL, WPARAM(SB_BOTTOM.0 as usize), LPARAM(0));
                assert!(send(f.edit, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0 > down);
                send(f.edit, WM_VSCROLL, WPARAM(SB_TOP.0 as usize), LPARAM(0));
                assert_eq!(
                    send(f.edit, EM_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0,
                    0
                );
                assert_eq!(
                    (*ptr).value(),
                    value,
                    "scrolling must not edit the document"
                );
            }
        }
    }
    #[test]
    fn input_regression_focused_empty_field_has_no_hint_under_caret() {
        unsafe {
            let f = InputFixture::new(false);
            send(f.edit, WM_SETFOCUS, WPARAM(0), LPARAM(0));
            f.print();
            assert_eq!(
                f.non_background_pixels(false),
                0,
                "focused empty input must show a native caret without placeholder glyphs"
            );
        }
    }
    #[test]
    fn input_regression_typing_invalidates_the_entire_old_hint() {
        unsafe {
            let f = InputFixture::new(false);
            f.print();
            let _ = ValidateRect(Some(f.edit), None);
            send(f.edit, WM_CHAR, WPARAM(b'a' as usize), LPARAM(0));
            let mut dirty = RECT::default();
            let full = GetUpdateRect(f.edit, Some(&mut dirty), false).as_bool();
            assert!(
                full && dirty.left == 0
                    && dirty.top == 0
                    && dirty.right == f.width
                    && dirty.bottom == f.height,
                "typing must invalidate the full multiline hint, got {dirty:?}"
            );
        }
    }
    #[test]
    fn input_regression_composition_start_clears_the_entire_old_hint() {
        unsafe {
            let f = InputFixture::new(false);
            f.print();
            let _ = ValidateRect(Some(f.edit), None);
            send(f.edit, WM_IME_STARTCOMPOSITION, WPARAM(0), LPARAM(0));
            let mut dirty = RECT::default();
            let full = GetUpdateRect(f.edit, Some(&mut dirty), false).as_bool();
            assert!(
                full && dirty.right == f.width && dirty.bottom == f.height,
                "composition start must erase all placeholder lines, got {dirty:?}"
            );
        }
    }
    #[test]
    fn input_regression_insertion_and_composition_inherit_field_background() {
        unsafe {
            for dark in [false, true] {
                let f = InputFixture::new(dark);
                let mut format = CHARFORMAT2W::default();
                format.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
                send(
                    f.edit,
                    EM_GETCHARFORMAT,
                    WPARAM(SCF_DEFAULT as usize),
                    LPARAM((&mut format as *mut CHARFORMAT2W) as isize),
                );
                assert_ne!(
                    format.Base.dwMask.0 & CFM_BACKCOLOR.0,
                    0,
                    "native insertion format must explicitly specify its background"
                );
                assert_eq!(
                    format.Base.dwEffects.0 & CFE_AUTOBACKCOLOR.0,
                    0,
                    "composition must not fall back to the system white background"
                );
                assert_eq!(format.crBackColor, editor_surface(dark));
            }
        }
    }
    #[test]
    fn input_regression_focus_clear_ime_and_multiline_batch_stay_consistent() {
        unsafe {
            for dark in [false, true] {
                let f = InputFixture::new(dark);
                let ptr = GetWindowLongPtrW(f.dialog, GWLP_USERDATA) as *mut Editor;
                f.print();
                assert!(
                    f.non_background_pixels(dark) > 0,
                    "blurred empty input may display its hint"
                );
                send(f.edit, WM_SETFOCUS, WPARAM(0), LPARAM(0));
                f.print();
                assert_eq!(f.non_background_pixels(dark), 0);
                send(f.edit, WM_CHAR, WPARAM(b'a' as usize), LPARAM(0));
                assert_eq!((*ptr).value(), "a");
                assert_eq!((*ptr).count, 1);
                f.print();
                // The second placeholder line used to survive single-line input.
                let c = editor_surface(dark).0;
                let background = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
                for y in 35..f.height {
                    for x in 0..f.width {
                        let pixel = std::slice::from_raw_parts(
                            f.pixels.offset(((y * f.width + x) * 4) as isize),
                            4,
                        );
                        assert_eq!(
                            &pixel[..3],
                            &background,
                            "old second-line hint must not survive typing"
                        );
                    }
                }
                SetWindowTextW(f.edit, w!("")).unwrap();
                f.print();
                assert_eq!(f.non_background_pixels(dark), 0);
                send(f.edit, WM_KILLFOCUS, WPARAM(0), LPARAM(0));
                f.print();
                assert!(f.non_background_pixels(dark) > 0);
                send(f.edit, WM_SETFOCUS, WPARAM(0), LPARAM(0));
                send(f.edit, WM_IME_STARTCOMPOSITION, WPARAM(0), LPARAM(0));
                f.print();
                assert_eq!(f.non_background_pixels(dark), 0);
                (*ptr).save();
                assert!(
                    (*ptr).core.list().unwrap().is_empty(),
                    "composition confirmation must not submit a todo"
                );
                send(f.edit, WM_IME_ENDCOMPOSITION, WPARAM(0), LPARAM(0));
                let single_item = wide("第一条\r\n第一条的说明\r\n  ");
                send(
                    f.edit,
                    EM_REPLACESEL,
                    WPARAM(1),
                    LPARAM(single_item.as_ptr() as isize),
                );
                assert_eq!(
                    (*ptr).count,
                    1,
                    "ordinary line breaks and a trailing blank line must not add another item"
                );
                let pasted = wide("\r\n第二条\r\n第二条的说明");
                send(
                    f.edit,
                    EM_REPLACESEL,
                    WPARAM(1),
                    LPARAM(pasted.as_ptr() as isize),
                );
                assert_eq!(
                    (*ptr).value(),
                    "第一条\n第一条的说明\n  \n第二条\n第二条的说明"
                );
                assert_eq!((*ptr).count, 2);
                (*ptr).save();
                let todos = (*ptr).core.list().unwrap();
                assert_eq!(
                    todos.iter().map(|t| t.content.as_str()).collect::<Vec<_>>(),
                    ["第一条\n第一条的说明", "第二条\n第二条的说明"]
                );
            }
        }
    }
    fn assert_readable_on_desktop_backgrounds(pixels: &[u8], dark: bool) {
        fn luminance(rgb: [f32; 3]) -> f32 {
            let c = rgb.map(|v| {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            });
            c[0] * 0.2126 + c[1] * 0.7152 + c[2] * 0.0722
        }
        fn composite(pixel: &[u8], bg: [f32; 3]) -> [f32; 3] {
            let a = pixel[3] as f32 / 255.0;
            [
                pixel[2] as f32 / 255.0 + bg[0] * (1.0 - a),
                pixel[1] as f32 / 255.0 + bg[1] * (1.0 - a),
                pixel[0] as f32 / 255.0 + bg[2] * (1.0 - a),
            ]
        }
        for bg in [[0.0; 3], [1.0; 3], [0.03, 0.20, 0.78]] {
            let pane = luminance(composite(&pixels[(110 * 474 + 220) * 4..], bg));
            // Inspect actual rasterized body and secondary glyphs, not palette constants.
            for (ys, xs) in [(115..143, 80..270), (482..510, 41..190)] {
                let mut inks = vec![];
                for y in ys {
                    for x in xs.clone() {
                        let px = &pixels[(y * 474 + x) * 4..];
                        if px[3] >= 253 {
                            inks.push(luminance(composite(px, bg)));
                        }
                    }
                }
                let ink = if dark {
                    inks.into_iter().reduce(f32::max)
                } else {
                    inks.into_iter().reduce(f32::min)
                }
                .expect("glyph region must contain opaque text");
                let ratio = (ink.max(pane) + 0.05) / (ink.min(pane) + 0.05);
                assert!(
                    ratio >= 4.5,
                    "text contrast {ratio:.2} on {bg:?}, dark={dark}"
                );
            }
        }
    }
    #[test]
    fn drag_drop_uses_visible_positions_independent_of_paint_order() {
        let ids = [1, 2, 4, 5];
        let rows = [(5, 200.0), (3, 90.0), (4, 140.0)];
        assert_eq!(drag_destination(&ids, &rows, 3, 130.0, 2), 2);
        assert_eq!(drag_destination(&ids, &rows, 3, 180.0, 2), 3);
        assert_eq!(drag_destination(&ids, &rows, 3, 240.0, 2), 4);
        assert_eq!(drag_destination(&ids, &[], 3, 0.0, 2), 2);
    }
    #[test]
    fn row_motion_keeps_position_on_reorder_and_stops_after_removal() {
        let todo = Todo {
            id: 1,
            content: "一条待办".into(),
            status: Status::Pending,
            status_id: "pending".into(),
            status_name: "未完成".into(),
            status_color: crate::statuses::StatusColor::Gray,
            position: 1024,
            version: 1,
        };
        let mut m = Motion::new(&WindowState::default(), true);
        m.place(&todo, 64.0, 46.0);
        m.initialized = true;
        m.place(&todo, 110.0, 46.0);
        assert_eq!(m.rows[&1].y.value, 64.0);
        for _ in 0..60 {
            m.step(0.016);
        }
        assert_eq!(m.rows[&1].y.value, 110.0);
        assert!(!m.active());
        let row = m.rows.get_mut(&1).unwrap();
        row.retiring = true;
        row.alpha.set(0.0);
        for _ in 0..60 {
            m.step(0.016);
        }
        assert!(m.rows.is_empty());
        assert!(!m.active());
    }
    #[test]
    fn composer_uses_matching_opaque_field_and_rounded_surface() {
        unsafe {
            let dir = std::path::PathBuf::from(".scratch/qa-native-render");
            std::fs::create_dir_all(&dir).unwrap();
            let mut p = Painter::new().unwrap();
            for dark in [false, true] {
                p.begin(438, 513, 1.5).unwrap();
                draw_composer(&p, 292.0, 342.0, dark, false, "");
                let pixels = p.test_pixels().unwrap();
                let field = &pixels[(200 * 438 + 200) * 4..][..4];
                assert_eq!(
                    field,
                    if dark {
                        &[63, 47, 35, 255]
                    } else {
                        &[245, 237, 230, 255]
                    }
                );
                assert_eq!(pixels[3], 0);
                std::fs::write(
                    dir.join(if dark {
                        "composer-dark-438x513.bgra"
                    } else {
                        "composer-light-438x513.bgra"
                    }),
                    pixels,
                )
                .unwrap();
            }
        }
    }
    #[test]
    fn native_painter_preserves_alpha_and_builds_real_hit_regions() {
        unsafe {
            // Off-screen raster verification; no window is opened or controlled.
            let directory = std::path::PathBuf::from(".scratch/qa-native-render");
            std::fs::create_dir_all(&directory).unwrap();
            let core = Arc::new(
                Core::new(&directory.join(format!("{}.sqlite3", std::process::id()))).unwrap(),
            );
            let a = core.create("整理项目思路").unwrap();
            let b = core.create("完成首页设计").unwrap();
            core.update(b.id, None, Some(Status::InProgress), None)
                .unwrap();
            core.create("看一下接口文档\n核对返回字段").unwrap();
            let mut app = note_fixture(core.clone(), directory.clone());
            app.painter.begin(474, 549, 1.5).unwrap();
            app.draw(316.0, 366.0);
            let pixels = app.painter.test_pixels().unwrap();
            assert!(
                pixels.as_chunks::<4>().0.iter().any(|p| p[3] > 240),
                "text and rim must be visible"
            );
            assert_eq!(
                pixels[3], 1,
                "rounded corner grip must use only one alpha step"
            );
            let center_alpha = pixels[(110 * 474 + 220) * 4 + 3];
            assert!(
                (230..255).contains(&center_alpha),
                "default pane must mask busy backgrounds while remaining slightly translucent, got {center_alpha}"
            );
            assert!(app.hits.iter().any(|h| h.action == Action::Check(a.id)));
            assert!(app.hits.iter().any(|h| h.action == Action::Progress(b.id)));
            assert!(app.hits.iter().any(|h| h.action == Action::Settings));
            assert_readable_on_desktop_backgrounds(&pixels, false);
            std::fs::write(directory.join("expanded-474x549.bgra"), pixels).unwrap();
            app.dark = true;
            app.painter.begin(474, 549, 1.5).unwrap();
            app.draw(316.0, 366.0);
            let pixels = app.painter.test_pixels().unwrap();
            assert_readable_on_desktop_backgrounds(&pixels, true);
            std::fs::write(directory.join("expanded-dark-474x549.bgra"), pixels).unwrap();
            app.appearance.text_size = 18;
            app.appearance.radius = 8;
            app.appearance.opacity = 100;
            app.painter.set_text_size(18).unwrap();
            app.painter.begin(474, 549, 1.5).unwrap();
            app.draw(316.0, 366.0);
            let pixels = app.painter.test_pixels().unwrap();
            assert_eq!(
                pixels[(22 * 474 + 22) * 4 + 3],
                255,
                "small-radius opaque setting must reach the inset corner"
            );
            assert!(app.hits.iter().any(|h| h.action == Action::Check(a.id)));
            std::fs::write(directory.join("large-text-474x549.bgra"), pixels).unwrap();
            app.state.collapsed = true;
            app.painter.begin(432, 129, 1.5).unwrap();
            app.draw(288.0, 86.0);
            let pixels = app.painter.test_pixels().unwrap();
            assert!(!app.hits.iter().any(|h| matches!(h.action, Action::Row(_))));
            std::fs::write(directory.join("collapsed-432x129.bgra"), pixels).unwrap();
        }
    }
    #[test]
    fn status_label_ink_is_centered_inside_the_actual_badge() {
        unsafe {
            let directory = std::path::PathBuf::from(".scratch/qa-status-centering");
            std::fs::create_dir_all(&directory).unwrap();
            let core = Arc::new(
                Core::new(&directory.join(format!("{}.sqlite3", std::process::id()))).unwrap(),
            );
            let item = core.create("标签居中验证").unwrap();
            core.update(item.id, None, Some(Status::InProgress), None)
                .unwrap();
            let mut app = note_fixture(core, directory);
            app.painter.begin(474, 549, 1.5).unwrap();
            app.draw(316.0, 366.0);
            let pixels = app.painter.test_pixels().unwrap();
            let badge = app
                .hits
                .iter()
                .find(|hit| hit.action == Action::Progress(item.id))
                .unwrap()
                .r;
            let mut ink = Vec::new();
            for y in (badge.y * 1.5).ceil() as usize..((badge.y + badge.h) * 1.5).floor() as usize {
                for x in
                    (badge.x * 1.5).ceil() as usize..((badge.x + badge.w) * 1.5).floor() as usize
                {
                    let p = &pixels[(y * 474 + x) * 4..];
                    if p[3] >= 254
                        && i32::from(p[0]) - i32::from(p[1]) > 20
                        && i32::from(p[1]) - i32::from(p[2]) > 20
                    {
                        ink.push((x, y));
                    }
                }
            }
            assert!(!ink.is_empty());
            let center_x = (ink.iter().map(|p| p.0).min().unwrap()
                + ink.iter().map(|p| p.0).max().unwrap()
                + 1) as f32
                / 2.0;
            let center_y = (ink.iter().map(|p| p.1).min().unwrap()
                + ink.iter().map(|p| p.1).max().unwrap()
                + 1) as f32
                / 2.0;
            assert!(
                (center_x - (badge.x + badge.w / 2.0) * 1.5).abs() <= 2.0,
                "horizontal ink center {center_x} must match badge center {}",
                (badge.x + badge.w / 2.0) * 1.5
            );
            assert!(
                (center_y - (badge.y + badge.h / 2.0) * 1.5).abs() <= 3.0,
                "vertical ink center {center_y} must match badge center {}",
                (badge.y + badge.h / 2.0) * 1.5
            );
        }
    }
    #[test]
    fn custom_status_badges_align_to_multiline_rows_without_button_overlap() {
        unsafe {
            let directory = std::path::PathBuf::from(".scratch/qa-custom-badges");
            std::fs::create_dir_all(&directory).unwrap();
            let core = Arc::new(
                Core::new(&directory.join(format!("{}.sqlite3", std::process::id()))).unwrap(),
            );
            let custom = core
                .create_status(
                    "等待产品反馈",
                    Status::InProgress,
                    crate::statuses::StatusColor::Purple,
                )
                .unwrap();
            let finished = core
                .create_status("已归档", Status::Done, crate::statuses::StatusColor::Green)
                .unwrap();
            let item = core
                .create("一条包含多行的事项\n第二行补充说明\n第三行正文")
                .unwrap();
            let done = core.create("已完成的记录").unwrap();
            core.update_with_status(item.id, None, None, Some(&custom.id), None)
                .unwrap();
            core.update_with_status(done.id, None, None, Some(&finished.id), None)
                .unwrap();
            let mut app = note_fixture(core, directory.clone());
            app.state.completed_open = true;
            app.motion.completed.snap(1.0);
            app.painter.begin(474, 900, 1.5).unwrap();
            app.draw(316.0, 600.0);
            let pixels = app.painter.test_pixels().unwrap();
            for id in [item.id, done.id] {
                let row = app
                    .hits
                    .iter()
                    .find(|h| h.action == Action::Row(id))
                    .unwrap()
                    .r;
                let badge = app
                    .hits
                    .iter()
                    .find(|h| h.action == Action::Progress(id))
                    .unwrap()
                    .r;
                let delete = app
                    .hits
                    .iter()
                    .find(|h| h.action == Action::Delete(id))
                    .unwrap()
                    .r;
                assert!((badge.y + badge.h / 2.0 - row.y - row.h / 2.0).abs() < 0.1);
                assert!((delete.y + delete.h / 2.0 - row.y - row.h / 2.0).abs() < 0.1);
                assert!(badge.x + badge.w < delete.x);
                assert!(badge.x > 51.0 + 64.0);
            }
            std::fs::write(directory.join("custom-labels-474x900.bgra"), pixels).unwrap();
        }
    }
}
