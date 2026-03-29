use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tracing::{debug, info};

use crate::error::{OrchestratorError, Result};

const DB_FILE_NAME: &str = "cflx.db";
const SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct ChangeEventRow {
    pub id: i64,
    pub project_id: String,
    pub change_id: String,
    pub run_id: Option<i64>,
    pub operation: String,
    pub attempt: i64,
    pub success: bool,
    pub duration_ms: i64,
    pub exit_code: Option<i64>,
    pub error: Option<String>,
    pub stdout_tail: Option<String>,
    pub stderr_tail: Option<String>,
    pub findings: Option<String>,
    pub commit_hash: Option<String>,
    pub verification_result: Option<String>,
    pub continuation_reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEntryRow {
    pub id: i64,
    pub project_id: Option<String>,
    pub level: String,
    pub message: String,
    pub change_id: Option<String>,
    pub operation: Option<String>,
    pub iteration: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ChangeStateRow {
    pub project_id: String,
    pub change_id: String,
    pub selected: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsOverview {
    pub success_count: i64,
    pub failure_count: i64,
    pub average_duration_ms: f64,
}

pub struct ServerDb {
    conn: Mutex<Connection>,
}

impl ServerDb {
    pub fn new(data_dir: &Path) -> Result<Arc<Self>> {
        std::fs::create_dir_all(data_dir).map_err(|e| {
            OrchestratorError::Io(std::io::Error::other(format!(
                "Failed to create server data dir '{}': {}",
                data_dir.display(),
                e
            )))
        })?;

        let db_path = data_dir.join(DB_FILE_NAME);
        let conn = Connection::open(&db_path).map_err(|e| {
            OrchestratorError::ConfigLoad(format!(
                "Failed to open sqlite db '{}': {}",
                db_path.display(),
                e
            ))
        })?;

        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| {
                OrchestratorError::ConfigLoad(format!("Failed to enable WAL mode: {}", e))
            })?;

        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(|e| {
                OrchestratorError::ConfigLoad(format!("Failed to enable foreign_keys: {}", e))
            })?;

        let db = Arc::new(Self {
            conn: Mutex::new(conn),
        });
        db.apply_migrations()?;

        info!(path = %db_path.display(), "Initialized server sqlite database");
        Ok(db)
    }

    fn with_conn<T>(&self, f: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> Result<T> {
        let conn = self.conn.lock().map_err(|_| {
            OrchestratorError::ConfigLoad("Failed to lock sqlite connection".to_string())
        })?;
        f(&conn)
            .map_err(|e| OrchestratorError::ConfigLoad(format!("SQLite operation failed: {}", e)))
    }

    fn apply_migrations(&self) -> Result<()> {
        self.with_conn(|conn| {
            let current_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;

            if current_version < 1 {
                conn.execute_batch(
                    "
                    CREATE TABLE IF NOT EXISTS orchestration_runs (
                        id            INTEGER PRIMARY KEY AUTOINCREMENT,
                        started_at    TEXT NOT NULL,
                        stopped_at    TEXT,
                        status        TEXT NOT NULL DEFAULT 'running',
                        trigger       TEXT
                    );

                    CREATE TABLE IF NOT EXISTS change_events (
                        id            INTEGER PRIMARY KEY AUTOINCREMENT,
                        project_id    TEXT NOT NULL,
                        change_id     TEXT NOT NULL,
                        run_id        INTEGER REFERENCES orchestration_runs(id),
                        operation     TEXT NOT NULL,
                        attempt       INTEGER NOT NULL,
                        success       INTEGER NOT NULL,
                        duration_ms   INTEGER NOT NULL,
                        exit_code     INTEGER,
                        error         TEXT,
                        stdout_tail   TEXT,
                        stderr_tail   TEXT,
                        findings      TEXT,
                        commit_hash   TEXT,
                        verification_result TEXT,
                        continuation_reason TEXT,
                        created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    );
                    CREATE INDEX IF NOT EXISTS idx_change_events_project_change ON change_events(project_id, change_id);
                    CREATE INDEX IF NOT EXISTS idx_change_events_created ON change_events(created_at);

                    CREATE TABLE IF NOT EXISTS log_entries (
                        id            INTEGER PRIMARY KEY AUTOINCREMENT,
                        project_id    TEXT,
                        level         TEXT NOT NULL,
                        message       TEXT NOT NULL,
                        change_id     TEXT,
                        operation     TEXT,
                        iteration     INTEGER,
                        created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    );
                    CREATE INDEX IF NOT EXISTS idx_log_entries_project ON log_entries(project_id);
                    CREATE INDEX IF NOT EXISTS idx_log_entries_created ON log_entries(created_at);

                    CREATE TABLE IF NOT EXISTS change_states (
                        project_id    TEXT NOT NULL,
                        change_id     TEXT NOT NULL,
                        selected      INTEGER NOT NULL DEFAULT 1,
                        error_message TEXT,
                        updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
                        PRIMARY KEY (project_id, change_id)
                    );
                    ",
                )?;
                conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            }

            Ok(())
        })
    }

    pub fn insert_run(&self, trigger: Option<&str>) -> Result<i64> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO orchestration_runs (started_at, status, trigger) VALUES (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), 'running', ?1)",
                params![trigger],
            )?;
            Ok(conn.last_insert_rowid())
        })
    }

    pub fn update_run_status(&self, run_id: i64, status: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE orchestration_runs SET status = ?1, stopped_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?2",
                params![status, run_id],
            )?;
            Ok(())
        })
    }

    pub fn get_recent_runs(
        &self,
        limit: usize,
    ) -> Result<Vec<(i64, String, Option<String>, String)>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, started_at, stopped_at, status FROM orchestration_runs ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit as i64], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_change_event(
        &self,
        project_id: &str,
        change_id: &str,
        run_id: Option<i64>,
        operation: &str,
        attempt: i64,
        success: bool,
        duration_ms: i64,
        exit_code: Option<i64>,
        error: Option<&str>,
        stdout_tail: Option<&str>,
        stderr_tail: Option<&str>,
        findings: Option<&str>,
        commit_hash: Option<&str>,
        verification_result: Option<&str>,
        continuation_reason: Option<&str>,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO change_events (
                    project_id, change_id, run_id, operation, attempt, success, duration_ms, exit_code,
                    error, stdout_tail, stderr_tail, findings, commit_hash, verification_result, continuation_reason
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                params![
                    project_id,
                    change_id,
                    run_id,
                    operation,
                    attempt,
                    if success { 1 } else { 0 },
                    duration_ms,
                    exit_code,
                    error,
                    stdout_tail,
                    stderr_tail,
                    findings,
                    commit_hash,
                    verification_result,
                    continuation_reason
                ],
            )?;
            Ok(())
        })
    }

    pub fn get_events_by_project_change(
        &self,
        project_id: &str,
        change_id: &str,
    ) -> Result<Vec<ChangeEventRow>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, change_id, run_id, operation, attempt, success, duration_ms, exit_code,
                        error, stdout_tail, stderr_tail, findings, commit_hash, verification_result, continuation_reason, created_at
                 FROM change_events WHERE project_id = ?1 AND change_id = ?2 ORDER BY id DESC",
            )?;
            let rows = stmt
                .query_map(params![project_id, change_id], |row| {
                    Ok(ChangeEventRow {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        change_id: row.get(2)?,
                        run_id: row.get(3)?,
                        operation: row.get(4)?,
                        attempt: row.get(5)?,
                        success: row.get::<_, i64>(6)? == 1,
                        duration_ms: row.get(7)?,
                        exit_code: row.get(8)?,
                        error: row.get(9)?,
                        stdout_tail: row.get(10)?,
                        stderr_tail: row.get(11)?,
                        findings: row.get(12)?,
                        commit_hash: row.get(13)?,
                        verification_result: row.get(14)?,
                        continuation_reason: row.get(15)?,
                        created_at: row.get(16)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn get_recent_events(&self, project_id: &str, limit: usize) -> Result<Vec<ChangeEventRow>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, change_id, run_id, operation, attempt, success, duration_ms, exit_code,
                        error, stdout_tail, stderr_tail, findings, commit_hash, verification_result, continuation_reason, created_at
                 FROM change_events WHERE project_id = ?1 ORDER BY id DESC LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(params![project_id, limit as i64], |row| {
                    Ok(ChangeEventRow {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        change_id: row.get(2)?,
                        run_id: row.get(3)?,
                        operation: row.get(4)?,
                        attempt: row.get(5)?,
                        success: row.get::<_, i64>(6)? == 1,
                        duration_ms: row.get(7)?,
                        exit_code: row.get(8)?,
                        error: row.get(9)?,
                        stdout_tail: row.get(10)?,
                        stderr_tail: row.get(11)?,
                        findings: row.get(12)?,
                        commit_hash: row.get(13)?,
                        verification_result: row.get(14)?,
                        continuation_reason: row.get(15)?,
                        created_at: row.get(16)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn get_stats_overview(&self) -> Result<StatsOverview> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as success_count,
                    SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) as failure_count,
                    COALESCE(AVG(duration_ms), 0) as average_duration_ms
                 FROM change_events",
            )?;
            let row = stmt.query_row([], |row| {
                Ok(StatsOverview {
                    success_count: row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                    failure_count: row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    average_duration_ms: row.get::<_, f64>(2)?,
                })
            })?;
            Ok(row)
        })
    }

    pub fn insert_log(
        &self,
        project_id: Option<&str>,
        level: &str,
        message: &str,
        change_id: Option<&str>,
        operation: Option<&str>,
        iteration: Option<i64>,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO log_entries (project_id, level, message, change_id, operation, iteration)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![project_id, level, message, change_id, operation, iteration],
            )?;
            Ok(())
        })
    }

    pub fn query_logs(
        &self,
        limit: usize,
        before: Option<&str>,
        project_id: Option<&str>,
    ) -> Result<Vec<LogEntryRow>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, level, message, change_id, operation, iteration, created_at
                 FROM log_entries
                 WHERE (?1 IS NULL OR project_id = ?1)
                   AND (?2 IS NULL OR created_at < ?2)
                 ORDER BY id DESC
                 LIMIT ?3",
            )?;
            let rows = stmt
                .query_map(params![project_id, before, limit as i64], |row| {
                    Ok(LogEntryRow {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        level: row.get(2)?,
                        message: row.get(3)?,
                        change_id: row.get(4)?,
                        operation: row.get(5)?,
                        iteration: row.get(6)?,
                        created_at: row.get(7)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn upsert_change_state(
        &self,
        project_id: &str,
        change_id: &str,
        selected: bool,
        error_message: Option<&str>,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO change_states (project_id, change_id, selected, error_message, updated_at)
                 VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ON CONFLICT(project_id, change_id)
                 DO UPDATE SET selected=excluded.selected, error_message=excluded.error_message,
                               updated_at=strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![project_id, change_id, if selected { 1 } else { 0 }, error_message],
            )?;
            Ok(())
        })
    }

    pub fn load_change_states(&self) -> Result<Vec<ChangeStateRow>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT project_id, change_id, selected, error_message FROM change_states",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(ChangeStateRow {
                        project_id: row.get(0)?,
                        change_id: row.get(1)?,
                        selected: row.get::<_, i64>(2)? == 1,
                        error_message: row.get(3)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn delete_change_states_for_project(&self, project_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM change_states WHERE project_id = ?1",
                params![project_id],
            )?;
            Ok(())
        })
    }

    pub fn cleanup_old_logs(&self, days: u32) -> Result<usize> {
        self.with_conn(|conn| {
            let count = conn.execute(
                "DELETE FROM log_entries WHERE created_at < datetime('now', '-' || ?1 || ' days')",
                params![days],
            )?;
            debug!(days, deleted = count, "Cleaned up old log entries");
            Ok(count)
        })
    }
}
