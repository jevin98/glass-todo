use crate::appearance::Appearance;
use crate::statuses::{StatusColor, StatusDefinition, custom_id};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, params};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    InProgress,
    Done,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Done => "done",
        }
    }
    pub(crate) fn parse(s: &str) -> Self {
        match s {
            "in_progress" => Self::InProgress,
            "done" => Self::Done,
            _ => Self::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Todo {
    pub id: i64,
    pub content: String,
    pub status: Status,
    pub status_id: String,
    pub status_name: String,
    pub status_color: StatusColor,
    pub position: i64,
    pub version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub width: i32,
    pub height: i32,
    pub pinned: bool,
    pub collapsed: bool,
    pub completed_open: bool,
    pub autostart: bool,
    pub dock: Option<DockAnchor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DockEdge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DockAnchor {
    pub edge: DockEdge,
    pub position: f32,
}
pub const DEFAULT_WIDTH: i32 = 316;
pub const DEFAULT_HEIGHT: i32 = 366;
impl Default for WindowState {
    fn default() -> Self {
        Self {
            x: None,
            y: None,
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            pinned: true,
            collapsed: false,
            completed_open: false,
            autostart: true,
            dock: None,
        }
    }
}

impl WindowState {
    pub fn normalized_size(mut self) -> Self {
        self.width = self.width.max(DEFAULT_WIDTH);
        self.height = self.height.max(DEFAULT_HEIGHT);
        if let Some(anchor) = &mut self.dock {
            anchor.position = if anchor.position.is_finite() {
                anchor.position.clamp(0.0, 1.0)
            } else {
                0.5
            };
        }
        self
    }
}

pub struct Store {
    conn: Connection,
}

fn content_checked(content: &str) -> Result<&str> {
    let content = content.trim();
    ensure!(!content.is_empty(), "待办内容不能为空");
    ensure!(
        content.chars().count() <= 16_384,
        "单条待办最多 16384 个字符"
    );
    ensure!(!content.contains('\0'), "内容不能包含空字符");
    Ok(content)
}

/// Composer drafts: a blank (including whitespace-only) line separates todos.
/// Ordinary line breaks remain part of the current todo's text.
pub fn split_todo_blocks(content: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut new_block = true;
    for line in content.lines() {
        if line.trim().is_empty() {
            new_block = true;
        } else if new_block {
            blocks.push(line.trim().to_owned());
            new_block = false;
        } else if let Some(block) = blocks.last_mut() {
            block.push('\n');
            block.push_str(line.trim_end());
        }
    }
    blocks
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let mut conn = Connection::open(path).context("打开待办数据库失败")?;
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
        // A single writer and small data: conservative durable rollback journal.
        conn.execute_batch(
            "PRAGMA journal_mode=DELETE; PRAGMA synchronous=EXTRA; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS todos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('pending','in_progress','done')),
                position INTEGER NOT NULL,
                version INTEGER NOT NULL DEFAULT 1,
                deleted_at INTEGER
            );
            CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )?;
        let migration = conn.transaction()?;
        migration.execute_batch(
            "CREATE TABLE IF NOT EXISTS custom_statuses (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL COLLATE NOCASE UNIQUE,
            kind TEXT NOT NULL CHECK(kind IN ('pending','in_progress','done')),
            color TEXT NOT NULL CHECK(color IN ('gray','blue','amber','green','purple','rose')),
            version INTEGER NOT NULL DEFAULT 1
        );",
        )?;
        let has_status_id = {
            let mut query = migration.prepare("PRAGMA table_info(todos)")?;
            query
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .iter()
                .any(|name| name == "custom_status_id")
        };
        if !has_status_id {
            migration.execute_batch("ALTER TABLE todos ADD COLUMN custom_status_id INTEGER REFERENCES custom_statuses(id);")?;
        }
        migration.execute_batch("PRAGMA user_version=2;")?;
        migration.commit()?;
        conn.execute(
            "INSERT OR IGNORE INTO settings(key,value) VALUES('appearance',?1)",
            [serde_json::to_string(&Appearance::default())?],
        )?;
        Ok(Self { conn })
    }

    pub fn list(&self) -> Result<Vec<Todo>> {
        let mut stmt = self.conn.prepare("SELECT t.id,t.content,t.status,t.position,t.version,s.id,s.name,s.color FROM todos t LEFT JOIN custom_statuses s ON t.custom_status_id=s.id WHERE t.deleted_at IS NULL ORDER BY t.position,t.id")?;
        Ok(stmt
            .query_map([], |row| {
                let status = Status::parse(&row.get::<_, String>(2)?);
                let builtin = StatusDefinition::builtin(status);
                let custom: Option<i64> = row.get(5)?;
                Ok(Todo {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    status,
                    status_id: custom
                        .map(|id| format!("custom_{id}"))
                        .unwrap_or(builtin.id),
                    status_name: row.get::<_, Option<String>>(6)?.unwrap_or(builtin.name),
                    status_color: row
                        .get::<_, Option<String>>(7)?
                        .map(|value| StatusColor::parse(&value))
                        .unwrap_or(builtin.color),
                    position: row.get(3)?,
                    version: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn get(&self, id: i64) -> Result<Todo> {
        self.list()?
            .into_iter()
            .find(|t| t.id == id)
            .context("事项不存在或已删除")
    }

    pub fn create(&mut self, content: &str) -> Result<Todo> {
        let content = content_checked(content)?;
        self.conn.execute("INSERT INTO todos(content,status,position) VALUES(?1,'pending',COALESCE((SELECT MAX(position)+1024 FROM todos),1024))", [content])?;
        self.get(self.conn.last_insert_rowid())
    }

    /// UI batch entry. Validate every blank-line-separated block before writing.
    pub fn create_batch(&mut self, content: &str) -> Result<usize> {
        let blocks = split_todo_blocks(content);
        ensure!(!blocks.is_empty(), "请先写下待办，多条之间空一行");
        for block in &blocks {
            content_checked(block)?;
        }
        let tx = self.conn.transaction()?;
        for block in &blocks {
            tx.execute("INSERT INTO todos(content,status,position) VALUES(?1,'pending',COALESCE((SELECT MAX(position)+1024 FROM todos),1024))", [block])?;
        }
        tx.commit()?;
        Ok(blocks.len())
    }

    pub fn update(
        &mut self,
        id: i64,
        content: Option<&str>,
        status: Option<Status>,
        expected: Option<i64>,
    ) -> Result<Todo> {
        self.update_with_status(id, content, status, None, expected)
    }
    pub fn update_with_status(
        &mut self,
        id: i64,
        content: Option<&str>,
        status: Option<Status>,
        status_id: Option<&str>,
        expected: Option<i64>,
    ) -> Result<Todo> {
        ensure!(
            status.is_none() || status_id.is_none(),
            "status 和 status_id 只指定一项"
        );
        let old = self.get(id)?;
        if let Some(v) = expected {
            ensure!(
                old.version == v,
                "版本冲突：当前版本为 {}，请重新查询后重试",
                old.version
            );
        }
        let content = match content {
            Some(c) => content_checked(c)?,
            None => &old.content,
        };
        let (status, custom) = if let Some(key) = status_id {
            let definition = self.status_definition(key)?;
            (definition.kind, custom_id(key))
        } else if let Some(status) = status {
            (status, None)
        } else {
            (old.status, custom_id(&old.status_id))
        };
        ensure!(self.conn.execute("UPDATE todos SET content=?1,status=?2,custom_status_id=?5,version=version+1 WHERE id=?3 AND version=?4 AND deleted_at IS NULL", params![content, status.as_str(), id, old.version,custom])? == 1, "事项已被其他操作修改，请重新查询");
        self.get(id)
    }

    pub fn delete(&mut self, id: i64, expected: Option<i64>) -> Result<()> {
        let old = self.get(id)?;
        if let Some(v) = expected {
            ensure!(old.version == v, "版本冲突，请重新查询后重试");
        }
        let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64;
        self.conn.execute(
            "UPDATE todos SET deleted_at=MAX(?1,COALESCE((SELECT MAX(deleted_at)+1 FROM todos),0)),version=version+1 WHERE id=?2 AND deleted_at IS NULL",
            params![now, id],
        )?;
        Ok(())
    }

    pub fn undo_delete(&mut self) -> Result<Todo> {
        let id: Option<i64> = self.conn.query_row("SELECT id FROM todos WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC,id DESC LIMIT 1", [], |r| r.get(0)).optional()?;
        let id = id.context("没有可撤销的删除")?;
        self.conn.execute(
            "UPDATE todos SET deleted_at=NULL,version=version+1 WHERE id=?1",
            [id],
        )?;
        self.get(id)
    }
    pub fn can_undo_delete(&self) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM todos WHERE deleted_at IS NOT NULL)",
            [],
            |r| r.get(0),
        )?)
    }

    pub fn reorder(&mut self, ids: &[i64]) -> Result<()> {
        let current: HashSet<i64> = self
            .list()?
            .iter()
            .filter(|t| t.status != Status::Done)
            .map(|t| t.id)
            .collect();
        let given: HashSet<i64> = ids.iter().copied().collect();
        ensure!(
            given.len() == ids.len() && given == current,
            "排序必须恰好包含全部未办妥事项 ID，不能重复或遗漏；请重新查询"
        );
        let tx = self.conn.transaction()?;
        for (i, id) in ids.iter().enumerate() {
            tx.execute(
                "UPDATE todos SET position=?1,version=version+1 WHERE id=?2",
                params![(i as i64 + 1) * 1024, id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn statuses(&self) -> Result<Vec<StatusDefinition>> {
        let mut definitions = [Status::Pending, Status::InProgress, Status::Done]
            .map(StatusDefinition::builtin)
            .to_vec();
        let mut stmt = self
            .conn
            .prepare("SELECT id,name,kind,color,version FROM custom_statuses ORDER BY id")?;
        for definition in stmt.query_map([], |row| {
            Ok(StatusDefinition {
                id: format!("custom_{}", row.get::<_, i64>(0)?),
                name: row.get(1)?,
                kind: Status::parse(&row.get::<_, String>(2)?),
                color: StatusColor::parse(&row.get::<_, String>(3)?),
                builtin: false,
                version: row.get(4)?,
            })
        })? {
            definitions.push(definition?);
        }
        Ok(definitions)
    }
    pub fn status_definition(&self, key: &str) -> Result<StatusDefinition> {
        self.statuses()?
            .into_iter()
            .find(|definition| definition.id == key)
            .context("状态不存在，请重新读取状态列表")
    }
    fn checked_status_name(&self, name: &str, except: Option<&str>) -> Result<String> {
        let name = name.trim();
        ensure!(
            !name.is_empty() && name.chars().count() <= 10 && !name.chars().any(char::is_control),
            "状态名称需要 1–10 个字符，不能换行"
        );
        let folded = name.to_lowercase();
        ensure!(
            !self
                .statuses()?
                .iter()
                .any(|definition| Some(definition.id.as_str()) != except
                    && definition.name.to_lowercase() == folded),
            "已有同名状态"
        );
        Ok(name.into())
    }
    pub fn create_status(
        &mut self,
        name: &str,
        kind: Status,
        color: StatusColor,
    ) -> Result<StatusDefinition> {
        ensure!(
            self.statuses()?.len() < 24,
            "最多保留 24 个状态，请先移除不再使用的状态"
        );
        let name = self.checked_status_name(name, None)?;
        self.conn.execute(
            "INSERT INTO custom_statuses(name,kind,color) VALUES(?1,?2,?3)",
            params![name, kind.as_str(), color.as_str()],
        )?;
        self.status_definition(&format!("custom_{}", self.conn.last_insert_rowid()))
    }
    pub fn update_status(
        &mut self,
        key: &str,
        name: Option<&str>,
        kind: Option<Status>,
        color: Option<StatusColor>,
        expected: Option<i64>,
    ) -> Result<StatusDefinition> {
        let old = self.status_definition(key)?;
        ensure!(!old.builtin, "内置状态固定，请新增自定义状态");
        if let Some(version) = expected {
            ensure!(version == old.version, "状态版本冲突，请重新读取后重试");
        }
        let name = self.checked_status_name(name.unwrap_or(&old.name), Some(key))?;
        let kind = kind.unwrap_or(old.kind);
        let color = color.unwrap_or(old.color);
        let id = custom_id(key).context("无效状态 ID")?;
        let tx = self.conn.transaction()?;
        ensure!(tx.execute("UPDATE custom_statuses SET name=?1,kind=?2,color=?3,version=version+1 WHERE id=?4 AND version=?5",params![name,kind.as_str(),color.as_str(),id,old.version])? == 1, "状态版本冲突，请重新读取后重试");
        if kind != old.kind {
            tx.execute(
                "UPDATE todos SET status=?1,version=version+1 WHERE custom_status_id=?2",
                params![kind.as_str(), id],
            )?;
        }
        tx.commit()?;
        self.status_definition(key)
    }
    pub fn delete_status(
        &mut self,
        key: &str,
        replacement: &str,
        expected: Option<i64>,
    ) -> Result<()> {
        let old = self.status_definition(key)?;
        ensure!(!old.builtin, "不能删除内置状态");
        ensure!(key != replacement, "替代状态不能是正在删除的状态");
        if let Some(version) = expected {
            ensure!(version == old.version, "状态版本冲突，请重新读取后重试");
        }
        let target = self.status_definition(replacement)?;
        let id = custom_id(key).context("无效状态 ID")?;
        let tx = self.conn.transaction()?;
        // Include soft-deleted records so undo never restores a missing status.
        tx.execute("UPDATE todos SET status=?1,custom_status_id=?2,version=version+1 WHERE custom_status_id=?3",params![target.kind.as_str(),custom_id(replacement),id])?;
        ensure!(
            tx.execute(
                "DELETE FROM custom_statuses WHERE id=?1 AND version=?2",
                params![id, old.version]
            )? == 1,
            "状态版本冲突，请重新读取后重试"
        );
        tx.commit()?;
        Ok(())
    }

    pub fn window(&self) -> Result<WindowState> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT value FROM settings WHERE key='window'", [], |r| {
                r.get(0)
            })
            .optional()?;
        match raw {
            Some(v) => Ok(serde_json::from_str::<WindowState>(&v)
                .unwrap_or_default()
                .normalized_size()),
            None => Ok(WindowState::default()),
        }
    }

    pub fn save_window(&self, state: &WindowState) -> Result<()> {
        self.conn.execute("INSERT INTO settings(key,value) VALUES('window',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [serde_json::to_string(&state.clone().normalized_size())?])?;
        Ok(())
    }
    pub fn appearance(&self) -> Result<Appearance> {
        let raw: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM settings WHERE key='appearance'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        Ok(raw
            .and_then(|s| serde_json::from_str::<Appearance>(&s).ok())
            .unwrap_or_default()
            .normalized())
    }
    pub fn save_appearance(&self, appearance: &Appearance) -> Result<()> {
        self.conn.execute("INSERT INTO settings(key,value) VALUES('appearance',?1) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [serde_json::to_string(&appearance.clone().normalized())?])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn legacy_status_migration_preserves_content_versions_and_undo() {
        let (path, store) = database();
        drop(store);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("DROP TABLE todos;DROP TABLE custom_statuses;
            CREATE TABLE todos(id INTEGER PRIMARY KEY AUTOINCREMENT,content TEXT NOT NULL,status TEXT NOT NULL CHECK(status IN ('pending','in_progress','done')),position INTEGER NOT NULL,version INTEGER NOT NULL DEFAULT 1,deleted_at INTEGER);
            INSERT INTO todos VALUES(41,'已有内容','in_progress',1024,7,NULL);
            INSERT INTO todos VALUES(42,'可恢复内容','done',2048,9,123);
            PRAGMA user_version=1;").unwrap();
        drop(conn);
        let mut store = Store::open(&path).unwrap();
        let item = store.get(41).unwrap();
        assert_eq!(
            (item.content.as_str(), item.version, item.status_id.as_str()),
            ("已有内容", 7, "in_progress")
        );
        assert_eq!(store.statuses().unwrap().len(), 3);
        let restored = store.undo_delete().unwrap();
        assert_eq!(
            (restored.id, restored.content.as_str(), restored.status),
            (42, "可恢复内容", Status::Done)
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn custom_statuses_survive_reopen_edit_conflicts_and_atomic_reassignment() {
        let (path, mut store) = database();
        let definition = store
            .create_status("待确认", Status::InProgress, StatusColor::Purple)
            .unwrap();
        let a = store.create("正文不能丢失").unwrap();
        let b = store.create("删除后仍保留状态").unwrap();
        let a = store
            .update_with_status(a.id, None, None, Some(&definition.id), Some(a.version))
            .unwrap();
        let b = store
            .update_with_status(b.id, None, None, Some(&definition.id), Some(b.version))
            .unwrap();
        assert_eq!(a.status_id, definition.id);
        assert_eq!(a.status_name, "待确认");
        assert_eq!(a.status_color, StatusColor::Purple);
        let edited = store
            .update(a.id, Some("更新正文"), None, Some(a.version))
            .unwrap();
        assert_eq!(
            edited.status_id, definition.id,
            "editing content must preserve the custom status"
        );
        store.delete(b.id, Some(b.version)).unwrap();
        drop(store);
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.get(a.id).unwrap().status_id, definition.id);
        assert!(
            store
                .update_status(&definition.id, Some("冲突名称"), None, None, Some(0))
                .is_err()
        );
        let changed = store
            .update_status(
                &definition.id,
                Some("已验收"),
                Some(Status::Done),
                Some(StatusColor::Green),
                Some(definition.version),
            )
            .unwrap();
        let after = store.get(a.id).unwrap();
        assert_eq!(after.status, Status::Done);
        assert_eq!(after.version, edited.version + 1);
        assert_eq!(after.status_name, "已验收");
        assert!(
            store
                .update(a.id, Some("不可覆盖"), None, Some(edited.version))
                .is_err()
        );
        assert!(
            store
                .delete_status(&changed.id, "missing", Some(changed.version))
                .is_err()
        );
        assert_eq!(store.get(a.id).unwrap().status_id, changed.id);
        store
            .delete_status(&changed.id, "pending", Some(changed.version))
            .unwrap();
        assert_eq!(
            (
                store.get(a.id).unwrap().content.as_str(),
                store.get(a.id).unwrap().status_id.as_str()
            ),
            ("更新正文", "pending")
        );
        let restored = store.undo_delete().unwrap();
        assert_eq!(
            (
                restored.id,
                restored.status_id.as_str(),
                restored.content.as_str()
            ),
            (b.id, "pending", "删除后仍保留状态")
        );
        assert!(store.status_definition(&definition.id).is_err());
        let next = store
            .create_status("待确认", Status::Pending, StatusColor::Amber)
            .unwrap();
        assert_ne!(
            next.id, definition.id,
            "deleted status IDs must not be reused"
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn custom_status_validation_keeps_builtin_ids_and_rejects_invalid_writes() {
        let (path, mut store) = database();
        for name in ["", "进行中", "  ", "一二三四五六七八九十一", "a\nb"] {
            assert!(
                store
                    .create_status(name, Status::Pending, StatusColor::Gray)
                    .is_err()
            );
        }
        let definition = store
            .create_status("Review", Status::InProgress, StatusColor::Blue)
            .unwrap();
        assert!(
            store
                .create_status("review", Status::InProgress, StatusColor::Blue)
                .is_err()
        );
        assert!(store.delete_status("done", "pending", Some(1)).is_err());
        assert!(
            store
                .update_status("pending", Some("可删除"), None, None, Some(1))
                .is_err()
        );
        let item = store.create("合法内容").unwrap();
        assert!(
            store
                .update_with_status(
                    item.id,
                    None,
                    None,
                    Some("custom_999999"),
                    Some(item.version)
                )
                .is_err()
        );
        assert!(
            store
                .update_with_status(
                    item.id,
                    None,
                    Some(Status::Done),
                    Some(&definition.id),
                    Some(item.version)
                )
                .is_err()
        );
        assert_eq!(store.get(item.id).unwrap().version, item.version);
        for index in 0..20 {
            store
                .create_status(&format!("状态{index}"), Status::Pending, StatusColor::Gray)
                .unwrap();
        }
        assert!(
            store
                .create_status("超额", Status::Pending, StatusColor::Gray)
                .is_err()
        );
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn docking_settings_preserve_full_size_and_migrate_older_records() {
        let (path, mut store) = database();
        store
            .conn
            .execute(
                "INSERT INTO settings(key,value) VALUES('window',?1)",
                [r#"{"width":316,"height":366,"pinned":false}"#],
            )
            .unwrap();
        assert!(store.window().unwrap().dock.is_none());
        for edge in [
            super::DockEdge::Left,
            super::DockEdge::Right,
            super::DockEdge::Top,
            super::DockEdge::Bottom,
        ] {
            let state = super::WindowState {
                width: 720,
                height: 640,
                x: Some(-1920),
                y: Some(120),
                dock: Some(super::DockAnchor {
                    edge,
                    position: 0.75,
                }),
                ..Default::default()
            };
            store.save_window(&state).unwrap();
            drop(store);
            store = super::Store::open(&path).unwrap();
            let restored = store.window().unwrap();
            assert_eq!(restored.dock, state.dock);
            assert_eq!((restored.width, restored.height), (720, 640));
            assert_eq!((restored.x, restored.y), (Some(-1920), Some(120)));
        }
        let state = super::WindowState {
            dock: Some(super::DockAnchor {
                edge: super::DockEdge::Left,
                position: 9.0,
            }),
            ..Default::default()
        };
        store.save_window(&state).unwrap();
        assert_eq!(store.window().unwrap().dock.unwrap().position, 1.0);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn resizing_keeps_default_minimum_and_preserves_larger_sizes() {
        let (path, store) = database();
        let small = super::WindowState {
            width: 280,
            height: 300,
            ..Default::default()
        };
        store.save_window(&small).unwrap();
        let restored = store.window().unwrap();
        assert_eq!((restored.width, restored.height), (316, 366));
        let large = super::WindowState {
            width: 1100,
            height: 1000,
            ..Default::default()
        }
        .normalized_size();
        store.save_window(&large).unwrap();
        drop(store);
        let store = super::Store::open(&path).unwrap();
        let restored = store.window().unwrap();
        assert_eq!((restored.width, restored.height), (1100, 1000));
        drop(store);
        std::fs::remove_file(path).unwrap();
    }
    use super::*;
    fn database() -> (std::path::PathBuf, Store) {
        static NEXT_TEST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "glass-todo-test-{}-{}-{}.sqlite3",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let s = Store::open(&p).unwrap();
        (p, s)
    }
    #[test]
    fn appearance_migrates_old_data_and_survives_other_settings_writes() {
        use crate::appearance::Theme;
        let (p, mut s) = database();
        let todo = s.create("已有待办").unwrap();
        assert_eq!(s.appearance().unwrap(), Appearance::default());
        s.conn
            .execute(
                "UPDATE settings SET value=?1 WHERE key='appearance'",
                [r#"{"theme":"dark","opacity":86}"#],
            )
            .unwrap();
        let mut a = s.appearance().unwrap();
        assert_eq!(a.theme, Theme::Dark);
        assert_eq!(a.text_size, 14);
        assert_eq!(a.radius, 28);
        a.text_size = 18;
        a.radius = 18;
        s.save_appearance(&a).unwrap();
        s.save_window(&WindowState {
            pinned: false,
            ..Default::default()
        })
        .unwrap();
        drop(s);
        let s = Store::open(&p).unwrap();
        assert_eq!(s.appearance().unwrap(), a);
        assert_eq!(s.get(todo.id).unwrap().content, "已有待办");
        s.save_appearance(&Appearance {
            opacity: 0,
            text_size: 255,
            radius: 255,
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            s.appearance().unwrap(),
            Appearance {
                opacity: 60,
                text_size: 18,
                radius: 32,
                ..Default::default()
            }
        );
        s.save_appearance(&Appearance::default()).unwrap();
        drop(s);
        let s = Store::open(&p).unwrap();
        assert_eq!(s.appearance().unwrap(), Appearance::default());
        drop(s);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn committed_multiline_and_state_survive_reopen() {
        let (p, mut s) = database();
        let t = s.create("第一行\n第二行").unwrap();
        s.update(t.id, None, Some(Status::InProgress), Some(t.version))
            .unwrap();
        let w = WindowState {
            x: Some(120),
            y: Some(80),
            pinned: false,
            ..Default::default()
        };
        s.save_window(&w).unwrap();
        drop(s);
        let s = Store::open(&p).unwrap();
        assert_eq!(s.get(t.id).unwrap().content, "第一行\n第二行");
        assert_eq!(s.get(t.id).unwrap().status, Status::InProgress);
        assert_eq!(s.window().unwrap().x, Some(120));
        drop(s);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn batch_blank_lines_separate_todos_and_preserve_multiline_text_atomically() {
        let (p, mut s) = database();
        assert_eq!(
            s.create_batch(
                "\r\n  第一条\r\n第一条的说明\r\n\r\n \t\r\n第二条\n  \t\u{3000}\n第三条  \n\n"
            )
            .unwrap(),
            3
        );
        assert!(s.create_batch("正常一条\n\n含\0空字符").is_err());
        assert!(s.create_batch(" \r\n\n ").is_err());
        let too_long = format!("正常一条\n\n{}\n{}", "文".repeat(9000), "字".repeat(9000));
        assert!(
            s.create_batch(&too_long).is_err(),
            "combined multiline text must respect the per-todo limit without partial writes"
        );
        drop(s);
        let mut s = Store::open(&p).unwrap();
        let list = s.list().unwrap();
        assert_eq!(
            list.iter().map(|t| t.content.as_str()).collect::<Vec<_>>(),
            ["第一条\n第一条的说明", "第二条", "第三条"]
        );
        assert!(
            list.windows(2)
                .all(|pair| pair[0].position < pair[1].position)
        );
        s.update(list[0].id, Some("编辑第一行\n编辑第二行"), None, None)
            .unwrap();
        assert_eq!(s.list().unwrap().len(), 3);
        assert_eq!(s.get(list[0].id).unwrap().content, "编辑第一行\n编辑第二行");
        drop(s);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn stale_agent_write_cannot_overwrite_newer_content() {
        let (p, mut s) = database();
        let t = s.create("old").unwrap();
        s.update(t.id, Some("new"), None, Some(1)).unwrap();
        assert!(s.update(t.id, Some("stale"), None, Some(1)).is_err());
        assert_eq!(s.get(t.id).unwrap().content, "new");
        drop(s);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn deleting_is_recoverable_and_invalid_reorder_is_atomic() {
        let (p, mut s) = database();
        let a = s.create("a").unwrap();
        let b = s.create("b").unwrap();
        assert!(s.reorder(&[a.id, a.id]).is_err());
        assert_eq!(s.list().unwrap()[0].id, a.id);
        s.reorder(&[b.id, a.id]).unwrap();
        assert_eq!(s.list().unwrap()[0].id, b.id);
        s.delete(a.id, None).unwrap();
        drop(s);
        let mut s = Store::open(&p).unwrap();
        assert_eq!(s.undo_delete().unwrap().id, a.id);
        assert_eq!(s.list().unwrap().len(), 2);
        drop(s);
        std::fs::remove_file(p).unwrap();
    }
}
