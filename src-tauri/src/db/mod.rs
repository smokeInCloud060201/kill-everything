use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    AppSettings, ExecutionLog, ExecutionResult, ImportMode, Script, ScriptExportEnvelope,
    ScriptExportItem, ScriptSummary, ScriptUpsert, SettingsPatch, ShellType,
};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(db_path: &Path) -> AppResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;
            PRAGMA busy_timeout = 5000;
            ",
        )?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn list_scripts(&self) -> AppResult<Vec<ScriptSummary>> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, name, icon, shell_type, run_elevated, confirm_before_run,
                   timeout_secs, is_pinned, sort_order, run_count, last_run_at, last_exit_code
            FROM scripts
            ORDER BY is_pinned DESC, sort_order ASC, name COLLATE NOCASE ASC
            ",
        )?;
        let rows = stmt.query_map([], map_script_summary)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn list_pinned_scripts(&self) -> AppResult<Vec<ScriptSummary>> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, name, icon, shell_type, run_elevated, confirm_before_run,
                   timeout_secs, is_pinned, sort_order, run_count, last_run_at, last_exit_code
            FROM scripts
            WHERE is_pinned = 1
            ORDER BY sort_order ASC, name COLLATE NOCASE ASC
            ",
        )?;
        let rows = stmt.query_map([], map_script_summary)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_script(&self, id: &str) -> AppResult<Script> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        conn.query_row(
            "
            SELECT id, name, icon, shell_type, custom_shell_path, script_content,
                   working_dir, run_elevated, confirm_before_run, timeout_secs,
                   is_pinned, sort_order, run_count, last_run_at, last_exit_code,
                   created_at, updated_at
            FROM scripts
            WHERE id = ?1
            ",
            params![id],
            map_script,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("Script '{id}' not found")))
    }

    pub fn save_script(&self, input: ScriptUpsert) -> AppResult<Script> {
        let name = input.name.trim().to_string();
        if name.is_empty() {
            return Err(AppError::Validation("Script name cannot be empty".into()));
        }
        if input.script_content.trim().is_empty() {
            return Err(AppError::Validation(
                "Script content cannot be empty".into(),
            ));
        }

        let custom_shell_path = clean_optional_text(input.custom_shell_path);
        if input.shell_type == ShellType::Custom && custom_shell_path.is_none() {
            return Err(AppError::Validation(
                "Custom shell path is required when shell is set to Custom".into(),
            ));
        }

        let icon = {
            let trimmed = input.icon.trim();
            if trimmed.is_empty() {
                "bolt".to_string()
            } else {
                trimmed.to_string()
            }
        };
        let working_dir = clean_optional_text(input.working_dir);
        let timeout_secs = input.timeout_secs.clamp(1, 300) as i64;
        let now = Utc::now().to_rfc3339();

        let id = {
            let conn = self.conn.lock().expect("db mutex poisoned");
            if let Some(existing_id) = input.id.filter(|s| !s.trim().is_empty()) {
                let updated = conn.execute(
                    "
                    UPDATE scripts
                    SET name = ?1,
                        icon = ?2,
                        shell_type = ?3,
                        custom_shell_path = ?4,
                        script_content = ?5,
                        working_dir = ?6,
                        run_elevated = ?7,
                        confirm_before_run = ?8,
                        timeout_secs = ?9,
                        is_pinned = ?10,
                        updated_at = ?11
                    WHERE id = ?12
                    ",
                    params![
                        name,
                        icon,
                        input.shell_type.as_str(),
                        custom_shell_path,
                        input.script_content,
                        working_dir,
                        bool_to_i64(input.run_elevated),
                        bool_to_i64(input.confirm_before_run),
                        timeout_secs,
                        bool_to_i64(input.is_pinned),
                        now,
                        existing_id
                    ],
                )?;
                if updated == 0 {
                    return Err(AppError::NotFound(format!(
                        "Script '{existing_id}' not found"
                    )));
                }
                existing_id
            } else {
                let new_id = Uuid::now_v7().to_string();
                let next_order: i64 = conn.query_row(
                    "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM scripts",
                    [],
                    |r| r.get(0),
                )?;
                conn.execute(
                    "
                    INSERT INTO scripts (
                        id, name, icon, shell_type, custom_shell_path, script_content,
                        working_dir, run_elevated, confirm_before_run, timeout_secs,
                        is_pinned, sort_order, run_count, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, ?13, ?14)
                    ",
                    params![
                        new_id,
                        name,
                        icon,
                        input.shell_type.as_str(),
                        custom_shell_path,
                        input.script_content,
                        working_dir,
                        bool_to_i64(input.run_elevated),
                        bool_to_i64(input.confirm_before_run),
                        timeout_secs,
                        bool_to_i64(input.is_pinned),
                        next_order,
                        now,
                        now
                    ],
                )?;
                new_id
            }
        };

        self.get_script(&id)
    }

    pub fn delete_script(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let deleted = conn.execute("DELETE FROM scripts WHERE id = ?1", params![id])?;
        if deleted == 0 {
            return Err(AppError::NotFound(format!("Script '{id}' not found")));
        }
        Ok(())
    }

    pub fn reorder_scripts(&self, ordered_ids: &[String]) -> AppResult<()> {
        let mut conn = self.conn.lock().expect("db mutex poisoned");
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE scripts SET sort_order = ?1 WHERE id = ?2")?;
            for (idx, id) in ordered_ids.iter().enumerate() {
                stmt.execute(params![idx as i64, id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn toggle_pin_script(&self, id: &str) -> AppResult<Script> {
        {
            let conn = self.conn.lock().expect("db mutex poisoned");
            let now = Utc::now().to_rfc3339();
            let updated = conn.execute(
                "
                UPDATE scripts
                SET is_pinned = CASE WHEN is_pinned = 0 THEN 1 ELSE 0 END,
                    updated_at = ?1
                WHERE id = ?2
                ",
                params![now, id],
            )?;
            if updated == 0 {
                return Err(AppError::NotFound(format!("Script '{id}' not found")));
            }
        }
        self.get_script(id)
    }

    pub fn record_execution(&self, script_id: &str, result: &ExecutionResult) -> AppResult<()> {
        let mut conn = self.conn.lock().expect("db mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "
            UPDATE scripts
            SET run_count = run_count + 1,
                last_run_at = ?1,
                last_exit_code = ?2
            WHERE id = ?3
            ",
            params![result.started_at, result.exit_code, script_id],
        )?;
        tx.execute(
            "
            INSERT INTO execution_logs (
                script_id, started_at, duration_ms, exit_code, timed_out, stdout, stderr
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                script_id,
                result.started_at,
                result.duration_ms,
                result.exit_code,
                bool_to_i64(result.timed_out),
                result.stdout,
                result.stderr,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_script_logs(&self, script_id: &str) -> AppResult<Vec<ExecutionLog>> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, script_id, started_at, duration_ms, exit_code, timed_out, stdout, stderr
            FROM execution_logs
            WHERE script_id = ?1
            ORDER BY id DESC
            LIMIT 10
            ",
        )?;
        let rows = stmt.query_map(params![script_id], |row| {
            Ok(ExecutionLog {
                id: row.get(0)?,
                script_id: row.get(1)?,
                started_at: row.get(2)?,
                duration_ms: row.get(3)?,
                exit_code: row.get(4)?,
                timed_out: row.get::<_, i64>(5)? != 0,
                stdout: row.get(6)?,
                stderr: row.get(7)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_settings(&self) -> AppResult<AppSettings> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut settings = AppSettings::default();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        for row in rows {
            let (key, val) = row?;
            match key.as_str() {
                "show_floating_tick" => {
                    if let Ok(v) = serde_json::from_str::<bool>(&val) {
                        settings.show_floating_tick = v;
                    }
                }
                "tick_x" => {
                    if let Ok(v) = serde_json::from_str::<Option<i32>>(&val) {
                        settings.tick_x = v;
                    }
                }
                "tick_y" => {
                    if let Ok(v) = serde_json::from_str::<Option<i32>>(&val) {
                        settings.tick_y = v;
                    }
                }
                "tick_monitor" => {
                    if let Ok(v) = serde_json::from_str::<Option<String>>(&val) {
                        settings.tick_monitor = v;
                    }
                }
                "tick_opacity" => {
                    if let Ok(v) = serde_json::from_str::<f64>(&val) {
                        settings.tick_opacity = v.clamp(0.2, 1.0);
                    }
                }
                "global_hotkey" => {
                    if let Ok(v) = serde_json::from_str::<Option<String>>(&val) {
                        settings.global_hotkey = v.filter(|s| !s.trim().is_empty());
                    }
                }
                "launch_at_login" => {
                    if let Ok(v) = serde_json::from_str::<bool>(&val) {
                        settings.launch_at_login = v;
                    }
                }
                "close_popover_on_kill" => {
                    if let Ok(v) = serde_json::from_str::<bool>(&val) {
                        settings.close_popover_on_kill = v;
                    }
                }
                _ => {}
            }
        }
        Ok(settings)
    }

    pub fn update_settings(&self, patch: SettingsPatch) -> AppResult<AppSettings> {
        let mut current = self.get_settings()?;
        if let Some(v) = patch.show_floating_tick {
            current.show_floating_tick = v;
        }
        if let Some(v) = patch.tick_x {
            current.tick_x = Some(v);
        }
        if let Some(v) = patch.tick_y {
            current.tick_y = Some(v);
        }
        if let Some(v) = patch.tick_monitor {
            current.tick_monitor = clean_optional_text(Some(v));
        }
        if let Some(v) = patch.tick_opacity {
            current.tick_opacity = v.clamp(0.2, 1.0);
        }
        if let Some(v) = patch.global_hotkey {
            let trimmed = v.trim().to_string();
            current.global_hotkey = if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            };
        }
        if let Some(v) = patch.launch_at_login {
            current.launch_at_login = v;
        }
        if let Some(v) = patch.close_popover_on_kill {
            current.close_popover_on_kill = v;
        }

        let pairs = [
            (
                "show_floating_tick",
                serde_json::to_string(&current.show_floating_tick)?,
            ),
            ("tick_x", serde_json::to_string(&current.tick_x)?),
            ("tick_y", serde_json::to_string(&current.tick_y)?),
            (
                "tick_monitor",
                serde_json::to_string(&current.tick_monitor)?,
            ),
            (
                "tick_opacity",
                serde_json::to_string(&current.tick_opacity)?,
            ),
            (
                "global_hotkey",
                serde_json::to_string(&current.global_hotkey)?,
            ),
            (
                "launch_at_login",
                serde_json::to_string(&current.launch_at_login)?,
            ),
            (
                "close_popover_on_kill",
                serde_json::to_string(&current.close_popover_on_kill)?,
            ),
        ];

        {
            let mut conn = self.conn.lock().expect("db mutex poisoned");
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "
                    INSERT INTO settings (key, value) VALUES (?1, ?2)
                    ON CONFLICT(key) DO UPDATE SET value = excluded.value
                    ",
                )?;
                for (key, val) in &pairs {
                    stmt.execute(params![key, val])?;
                }
            }
            tx.commit()?;
        }

        Ok(current)
    }

    pub fn export_scripts_json(&self) -> AppResult<String> {
        let conn = self.conn.lock().expect("db mutex poisoned");
        let mut stmt = conn.prepare(
            "
            SELECT id, name, icon, shell_type, custom_shell_path, script_content,
                   working_dir, run_elevated, confirm_before_run, timeout_secs,
                   is_pinned, sort_order, run_count, last_run_at, last_exit_code,
                   created_at, updated_at
            FROM scripts
            ORDER BY is_pinned DESC, sort_order ASC, name COLLATE NOCASE ASC
            ",
        )?;
        let rows = stmt.query_map([], map_script)?;
        let mut items = Vec::new();
        for row in rows {
            let s = row?;
            items.push(ScriptExportItem {
                name: s.name,
                icon: s.icon,
                shell_type: s.shell_type,
                custom_shell_path: s.custom_shell_path,
                script_content: s.script_content,
                working_dir: s.working_dir,
                run_elevated: s.run_elevated,
                confirm_before_run: s.confirm_before_run,
                timeout_secs: s.timeout_secs,
                is_pinned: s.is_pinned,
            });
        }
        let envelope = ScriptExportEnvelope {
            version: 1,
            exported_at: Utc::now().to_rfc3339(),
            scripts: items,
        };
        Ok(serde_json::to_string_pretty(&envelope)?)
    }

    pub fn import_scripts_json(&self, payload: &str, mode: ImportMode) -> AppResult<usize> {
        let envelope: ScriptExportEnvelope = serde_json::from_str(payload)?;
        if envelope.version != 1 {
            return Err(AppError::Validation(format!(
                "Unsupported export format version: {}",
                envelope.version
            )));
        }

        for item in &envelope.scripts {
            if item.name.trim().is_empty() || item.script_content.trim().is_empty() {
                return Err(AppError::Validation(
                    "Imported scripts must have non-empty name and scriptContent".into(),
                ));
            }
            if item.shell_type == ShellType::Custom
                && clean_optional_text(item.custom_shell_path.clone()).is_none()
            {
                return Err(AppError::Validation(format!(
                    "Script '{}' uses custom shell without customShellPath",
                    item.name
                )));
            }
        }

        let mut conn = self.conn.lock().expect("db mutex poisoned");
        let tx = conn.transaction()?;

        if mode == ImportMode::Replace {
            tx.execute("DELETE FROM scripts", [])?;
        }

        let mut next_order: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM scripts",
            [],
            |r| r.get(0),
        )?;
        let now = Utc::now().to_rfc3339();
        let count = envelope.scripts.len();

        {
            let mut stmt = tx.prepare(
                "
                INSERT INTO scripts (
                    id, name, icon, shell_type, custom_shell_path, script_content,
                    working_dir, run_elevated, confirm_before_run, timeout_secs,
                    is_pinned, sort_order, run_count, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, ?13, ?14)
                ",
            )?;
            for item in envelope.scripts {
                let id = Uuid::now_v7().to_string();
                let icon = {
                    let t = item.icon.trim();
                    if t.is_empty() {
                        "bolt".to_string()
                    } else {
                        t.to_string()
                    }
                };
                stmt.execute(params![
                    id,
                    item.name.trim(),
                    icon,
                    item.shell_type.as_str(),
                    clean_optional_text(item.custom_shell_path),
                    item.script_content,
                    clean_optional_text(item.working_dir),
                    bool_to_i64(item.run_elevated),
                    bool_to_i64(item.confirm_before_run),
                    item.timeout_secs.clamp(1, 300) as i64,
                    bool_to_i64(item.is_pinned),
                    next_order,
                    now,
                    now
                ])?;
                next_order += 1;
            }
        }

        tx.commit()?;
        Ok(count)
    }
}

fn map_script_summary(row: &Row<'_>) -> rusqlite::Result<ScriptSummary> {
    let shell_str: String = row.get(3)?;
    let shell_type = ShellType::from_str(&shell_str).unwrap_or(ShellType::Sh);
    Ok(ScriptSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        icon: row.get(2)?,
        shell_type,
        run_elevated: row.get::<_, i64>(4)? != 0,
        confirm_before_run: row.get::<_, i64>(5)? != 0,
        timeout_secs: (row.get::<_, i64>(6)?).clamp(1, 300) as u32,
        is_pinned: row.get::<_, i64>(7)? != 0,
        sort_order: row.get(8)?,
        run_count: row.get(9)?,
        last_run_at: row.get(10)?,
        last_exit_code: row.get(11)?,
    })
}

fn map_script(row: &Row<'_>) -> rusqlite::Result<Script> {
    let shell_str: String = row.get(3)?;
    let shell_type = ShellType::from_str(&shell_str).unwrap_or(ShellType::Sh);
    Ok(Script {
        id: row.get(0)?,
        name: row.get(1)?,
        icon: row.get(2)?,
        shell_type,
        custom_shell_path: row.get(4)?,
        script_content: row.get(5)?,
        working_dir: row.get(6)?,
        run_elevated: row.get::<_, i64>(7)? != 0,
        confirm_before_run: row.get::<_, i64>(8)? != 0,
        timeout_secs: (row.get::<_, i64>(9)?).clamp(1, 300) as u32,
        is_pinned: row.get::<_, i64>(10)? != 0,
        sort_order: row.get(11)?,
        run_count: row.get(12)?,
        last_run_at: row.get(13)?,
        last_exit_code: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn clean_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn bool_to_i64(b: bool) -> i64 {
    if b {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_db() -> (TempDir, Database) {
        let tmp = TempDir::new().unwrap();
        let db = Database::open(&tmp.path().join("data.db")).unwrap();
        (tmp, db)
    }

    fn make_upsert(name: &str, content: &str, pinned: bool) -> ScriptUpsert {
        ScriptUpsert {
            id: None,
            name: name.to_string(),
            icon: "bolt".to_string(),
            shell_type: ShellType::Bash,
            custom_shell_path: None,
            script_content: content.to_string(),
            working_dir: None,
            run_elevated: false,
            confirm_before_run: false,
            timeout_secs: 15,
            is_pinned: pinned,
        }
    }

    #[test]
    fn script_crud_ordering_and_pinning() {
        let (_tmp, db) = test_db();

        let s1 = db
            .save_script(make_upsert("Chrome", "pkill -f Chrome", false))
            .unwrap();
        let s2 = db
            .save_script(make_upsert("Docker", "pkill -f Docker", true))
            .unwrap();
        let s3 = db
            .save_script(make_upsert("Node", "pkill -f node", false))
            .unwrap();

        // Pinned script appears first
        let list = db.list_scripts().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].id, s2.id);
        assert_eq!(list[1].id, s1.id);
        assert_eq!(list[2].id, s3.id);

        // Reorder unpinned scripts
        db.reorder_scripts(&[s2.id.clone(), s3.id.clone(), s1.id.clone()])
            .unwrap();
        let list_reordered = db.list_scripts().unwrap();
        assert_eq!(list_reordered[1].id, s3.id);
        assert_eq!(list_reordered[2].id, s1.id);

        // Update existing script preserves raw whitespace/content
        let updated = db
            .save_script(ScriptUpsert {
                id: Some(s1.id.clone()),
                name: "Chrome Canary".to_string(),
                icon: "flame".to_string(),
                shell_type: ShellType::Zsh,
                custom_shell_path: None,
                script_content: "pkill -9 Chrome\n  echo done\n".to_string(),
                working_dir: None,
                run_elevated: true,
                confirm_before_run: true,
                timeout_secs: 30,
                is_pinned: true,
            })
            .unwrap();
        assert_eq!(updated.name, "Chrome Canary");
        assert_eq!(updated.script_content, "pkill -9 Chrome\n  echo done\n");
        assert!(updated.is_pinned);
        assert!(updated.run_elevated);

        // Toggle pin off
        let unpinned = db.toggle_pin_script(&s1.id).unwrap();
        assert!(!unpinned.is_pinned);

        // Delete script
        db.delete_script(&s3.id).unwrap();
        assert!(db.get_script(&s3.id).is_err());
        assert_eq!(db.list_scripts().unwrap().len(), 2);
    }

    #[test]
    fn execution_logs_prune_to_latest_ten_and_cascade_delete() {
        let (_tmp, db) = test_db();
        let s = db
            .save_script(make_upsert("Test", "echo hi", false))
            .unwrap();

        for idx in 1..=15 {
            db.record_execution(
                &s.id,
                &ExecutionResult {
                    script_id: Some(s.id.clone()),
                    started_at: format!("2026-01-01T00:00:{idx:02}Z"),
                    duration_ms: idx * 10,
                    exit_code: Some(0),
                    timed_out: false,
                    stdout: format!("run-{idx}"),
                    stderr: String::new(),
                    success: true,
                },
            )
            .unwrap();
        }

        let updated = db.get_script(&s.id).unwrap();
        assert_eq!(updated.run_count, 15);
        assert_eq!(updated.last_exit_code, Some(0));

        let logs = db.get_script_logs(&s.id).unwrap();
        assert_eq!(logs.len(), 10);
        assert_eq!(logs[0].stdout, "run-15");
        assert_eq!(logs[9].stdout, "run-6");

        db.delete_script(&s.id).unwrap();
        assert_eq!(db.get_script_logs(&s.id).unwrap().len(), 0);
    }

    #[test]
    fn settings_roundtrip_and_json_backup_import_export() {
        let (_tmp, db) = test_db();
        let initial = db.get_settings().unwrap();
        assert!(initial.show_floating_tick);
        assert_eq!(initial.global_hotkey, None);

        let patched = db
            .update_settings(SettingsPatch {
                show_floating_tick: Some(false),
                tick_x: Some(120),
                tick_y: Some(240),
                tick_monitor: Some("Built-in".into()),
                tick_opacity: Some(0.8),
                global_hotkey: Some("CommandOrControl+Shift+K".into()),
                launch_at_login: Some(true),
                close_popover_on_kill: Some(true),
            })
            .unwrap();
        assert!(!patched.show_floating_tick);
        assert_eq!(patched.tick_x, Some(120));
        assert_eq!(
            patched.global_hotkey.as_deref(),
            Some("CommandOrControl+Shift+K")
        );

        let cleared = db
            .update_settings(SettingsPatch {
                global_hotkey: Some(String::new()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(cleared.global_hotkey, None);

        db.save_script(make_upsert("A", "echo A", true)).unwrap();
        db.save_script(make_upsert("B", "echo B", false)).unwrap();

        let json = db.export_scripts_json().unwrap();
        let merged = db.import_scripts_json(&json, ImportMode::Merge).unwrap();
        assert_eq!(merged, 2);
        assert_eq!(db.list_scripts().unwrap().len(), 4);

        let replaced = db.import_scripts_json(&json, ImportMode::Replace).unwrap();
        assert_eq!(replaced, 2);
        assert_eq!(db.list_scripts().unwrap().len(), 2);
    }
}
