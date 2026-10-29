use crate::appearance::Appearance;
use crate::statuses::{StatusColor, StatusDefinition};
use crate::store::{Status, Store, Todo, WindowState};
use anyhow::{Context, Result};
use std::{
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicIsize, Ordering},
    },
};

pub const WM_REFRESH: u32 = 0x8000 + 7;
pub const WM_APPEARANCE: u32 = 0x8000 + 8;

pub struct Core {
    pub db: Mutex<Store>,
    pub hwnd: AtomicIsize,
    appearance: Mutex<Appearance>,
    refresh_pending: AtomicBool,
}

impl Core {
    pub fn new(path: &Path) -> Result<Self> {
        let db = Store::open(path)?;
        let appearance = db.appearance()?;
        Ok(Self {
            db: Mutex::new(db),
            appearance: Mutex::new(appearance),
            refresh_pending: AtomicBool::new(false),
            hwnd: AtomicIsize::new(0),
        })
    }
    fn read<T>(&self, f: impl FnOnce(&Store) -> Result<T>) -> Result<T> {
        let db = self
            .db
            .lock()
            .map_err(|_| anyhow::anyhow!("数据库锁不可用"))?;
        f(&db)
    }
    fn write<T>(&self, f: impl FnOnce(&mut Store) -> Result<T>) -> Result<T> {
        let result = {
            let mut db = self
                .db
                .lock()
                .map_err(|_| anyhow::anyhow!("数据库锁不可用"))?;
            f(&mut db)
        };
        if result.is_ok() {
            self.notify();
        }
        result
    }
    pub fn list(&self) -> Result<Vec<Todo>> {
        self.read(Store::list)
    }
    pub fn get(&self, id: i64) -> Result<Todo> {
        self.read(|s| s.get(id))
    }
    pub fn create(&self, content: &str) -> Result<Todo> {
        self.write(|s| s.create(content))
    }
    pub fn create_batch(&self, content: &str) -> Result<usize> {
        self.write(|s| s.create_batch(content))
    }
    pub fn update(
        &self,
        id: i64,
        content: Option<&str>,
        status: Option<Status>,
        expected: Option<i64>,
    ) -> Result<Todo> {
        self.write(|s| s.update(id, content, status, expected))
    }
    pub fn update_with_status(
        &self,
        id: i64,
        content: Option<&str>,
        status: Option<Status>,
        status_id: Option<&str>,
        expected: Option<i64>,
    ) -> Result<Todo> {
        self.write(|s| s.update_with_status(id, content, status, status_id, expected))
    }
    pub fn statuses(&self) -> Result<Vec<StatusDefinition>> {
        self.read(Store::statuses)
    }
    pub fn create_status(
        &self,
        name: &str,
        kind: Status,
        color: StatusColor,
    ) -> Result<StatusDefinition> {
        self.write(|s| s.create_status(name, kind, color))
    }
    pub fn update_status(
        &self,
        id: &str,
        name: Option<&str>,
        kind: Option<Status>,
        color: Option<StatusColor>,
        expected: Option<i64>,
    ) -> Result<StatusDefinition> {
        self.write(|s| s.update_status(id, name, kind, color, expected))
    }
    pub fn delete_status(&self, id: &str, replacement: &str, expected: Option<i64>) -> Result<()> {
        self.write(|s| s.delete_status(id, replacement, expected))
    }
    pub fn delete(&self, id: i64, expected: Option<i64>) -> Result<()> {
        self.write(|s| s.delete(id, expected))
    }
    pub fn undo(&self) -> Result<Todo> {
        self.write(Store::undo_delete)
    }
    pub fn can_undo_delete(&self) -> Result<bool> {
        self.read(Store::can_undo_delete)
    }
    pub fn reorder(&self, ids: &[i64]) -> Result<()> {
        self.write(|s| s.reorder(ids))
    }
    pub fn window(&self) -> WindowState {
        self.read(Store::window).unwrap_or_default()
    }
    pub fn save_window(&self, s: &WindowState) -> Result<()> {
        self.read(|db| db.save_window(s))
            .context("保存窗口位置失败")
    }
    pub fn notify(&self) {
        if !self.refresh_pending.swap(true, Ordering::AcqRel) && !self.notify_message(WM_REFRESH) {
            self.refresh_pending.store(false, Ordering::Release);
        }
    }
    pub fn refresh_started(&self) {
        self.refresh_pending.store(false, Ordering::Release);
    }
    pub fn appearance(&self) -> Appearance {
        self.appearance
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default()
    }
    pub fn preview_appearance(&self, appearance: &Appearance) {
        if let Ok(mut current) = self.appearance.lock() {
            *current = appearance.clone().normalized();
        }
        self.notify_message(WM_APPEARANCE);
    }
    pub fn save_appearance(&self, appearance: &Appearance) -> Result<()> {
        self.read(|db| db.save_appearance(appearance))
            .context("保存外观失败")
    }
    fn notify_message(&self, message: u32) -> bool {
        #[cfg(windows)]
        unsafe {
            use windows::Win32::{
                Foundation::{HWND, LPARAM, WPARAM},
                UI::WindowsAndMessaging::PostMessageW,
            };
            let hwnd = self.hwnd.load(Ordering::Acquire);
            if hwnd != 0 {
                return PostMessageW(Some(HWND(hwnd as *mut _)), message, WPARAM(0), LPARAM(0))
                    .is_ok();
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(windows)]
    fn repeated_writes_only_queue_one_refresh() {
        unsafe {
            use windows::{Win32::UI::WindowsAndMessaging::*, core::w};
            let path = std::env::temp_dir()
                .join(format!("glass-todo-refresh-{}.sqlite3", std::process::id()));
            let core = Core::new(&path).unwrap();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("private refresh probe"),
                WS_POPUP,
                0,
                0,
                100,
                100,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            core.hwnd.store(hwnd.0 as isize, Ordering::Release);
            for i in 0..100 {
                core.create(&format!("sample {i}")).unwrap();
            }
            let mut msg = MSG::default();
            let mut queued = 0;
            while PeekMessageW(&mut msg, Some(hwnd), WM_REFRESH, WM_REFRESH, PM_REMOVE).as_bool() {
                queued += 1;
            }
            core.hwnd.store(0, Ordering::Release);
            let _ = DestroyWindow(hwnd);
            assert_eq!(
                queued, 1,
                "multiple writes must coalesce before the UI reads the latest snapshot"
            );
        }
    }
    #[test]
    fn appearance_preview_is_separate_from_durable_save() {
        let path =
            std::env::temp_dir().join(format!("glass-todo-preview-{}.sqlite3", std::process::id()));
        let core = Core::new(&path).unwrap();
        let original = core.appearance();
        let changed = Appearance {
            opacity: 80,
            text_size: 16,
            radius: 16,
            ..original.clone()
        };
        core.preview_appearance(&changed);
        assert_eq!(core.appearance(), changed);
        assert_eq!(Store::open(&path).unwrap().appearance().unwrap(), original);
        core.save_appearance(&changed).unwrap();
        drop(core);
        let core = Core::new(&path).unwrap();
        assert_eq!(core.appearance(), changed);
        drop(core);
        std::fs::remove_file(path).unwrap();
    }
}
