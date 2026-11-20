CREATE TABLE IF NOT EXISTS scripts (
    id                 TEXT PRIMARY KEY,
    name               TEXT NOT NULL,
    icon               TEXT NOT NULL DEFAULT 'bolt',
    shell_type         TEXT NOT NULL,
    custom_shell_path  TEXT,
    script_content     TEXT NOT NULL,
    working_dir        TEXT,
    run_elevated       INTEGER NOT NULL DEFAULT 0,
    confirm_before_run INTEGER NOT NULL DEFAULT 0,
    timeout_secs       INTEGER NOT NULL DEFAULT 15,
    is_pinned          INTEGER NOT NULL DEFAULT 0,
    sort_order         INTEGER NOT NULL DEFAULT 0,
    run_count          INTEGER NOT NULL DEFAULT 0,
    last_run_at        TEXT,
    last_exit_code     INTEGER,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS execution_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    script_id   TEXT NOT NULL REFERENCES scripts(id) ON DELETE CASCADE,
    started_at  TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    exit_code   INTEGER,
    timed_out   INTEGER NOT NULL DEFAULT 0,
    stdout      TEXT NOT NULL DEFAULT '',
    stderr      TEXT NOT NULL DEFAULT ''
);

CREATE TRIGGER IF NOT EXISTS prune_execution_logs AFTER INSERT ON execution_logs
BEGIN
    DELETE FROM execution_logs
    WHERE script_id = NEW.script_id
      AND id NOT IN (
          SELECT id FROM execution_logs
          WHERE script_id = NEW.script_id
          ORDER BY id DESC LIMIT 10
      );
END;

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
