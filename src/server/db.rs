use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};
use serde::Serialize;
use tracing::{debug, info};

use crate::error::{OrchestratorError, Result};
use crate::server::proposal_session::ProposalSessionMessageRecord;

const DB_FILE_NAME: &str = "cflx.db";
const SCHEMA_VERSION: i64 = 2;

#[derive(Debug, Clone)]
pub struct ProposalSessionDbRow {
    pub id: String,
    pub project_id: String,
    pub worktree_path: String,
    pub worktree_branch: String,
    pub status: String,
    pub created_at: String,
    pub last_activity: String,
}

#[derive(Debug, Clone)]
pub struct ProposalSessionUpsert<'a> {
    pub id: &'a str,
    pub project_id: &'a str,
    pub worktree_path: &'a str,
    pub worktree_branch: &'a str,
    pub status: &'a str,
    pub created_at: &'a str,
    pub updated_at: &'a str,
    pub last_activity: &'a str,
}

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

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RunRow {
    pub id: i64,
    pub started_at: String,
    pub stopped_at: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ChangeStateRow {
    pub project_id: String,
    pub change_id: String,
    pub selected: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsOverviewSummary {
    pub success_count: i64,
    pub failure_count: i64,
    pub in_progress_count: i64,
    pub average_duration_ms: Option<f64>,
    pub average_duration_by_operation: Option<std::collections::HashMap<String, f64>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentEventSummary {
    pub project_id: String,
    pub change_id: String,
    pub operation: String,
    pub result: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatsSummary {
    pub project_id: String,
    pub apply_success_rate: f64,
    pub average_duration_ms: Option<f64>,
    pub success_count: i64,
    pub failure_count: i64,
    pub in_progress_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatsOverview {
    pub summary: StatsOverviewSummary,
    pub recent_events: Vec<RecentEventSummary>,
    pub project_stats: Vec<ProjectStatsSummary>,
}

pub struct ServerDb {
    conn: Mutex<Connection>,
}

#[allow(dead_code)]
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
                conn.pragma_update(None, "user_version", 1)?;
            }

            if current_version < 2 {
                conn.execute_batch(
                    "
                    CREATE TABLE IF NOT EXISTS ui_state (
                        key           TEXT PRIMARY KEY,
                        value         TEXT NOT NULL,
                        updated_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                    );

                    CREATE TABLE IF NOT EXISTS proposal_sessions (
                        id            TEXT PRIMARY KEY,
                        project_id    TEXT NOT NULL,
                        worktree_path TEXT NOT NULL,
                        worktree_branch TEXT NOT NULL,
                        status        TEXT NOT NULL,
                        created_at    TEXT NOT NULL,
                        updated_at    TEXT NOT NULL,
                        last_activity TEXT NOT NULL
                    );
                    CREATE INDEX IF NOT EXISTS idx_proposal_sessions_project ON proposal_sessions(project_id);
                    CREATE INDEX IF NOT EXISTS idx_proposal_sessions_status ON proposal_sessions(status);

                    CREATE TABLE IF NOT EXISTS proposal_session_messages (
                        id            INTEGER PRIMARY KEY AUTOINCREMENT,
                        session_id    TEXT NOT NULL REFERENCES proposal_sessions(id) ON DELETE CASCADE,
                        message_id    TEXT NOT NULL,
                        role          TEXT NOT NULL,
                        content       TEXT NOT NULL,
                        timestamp     TEXT NOT NULL,
                        turn_id       TEXT,
                        hydrated      INTEGER,
                        is_thought    INTEGER,
                        tool_calls_json TEXT
                    );
                    CREATE UNIQUE INDEX IF NOT EXISTS idx_proposal_session_messages_unique
                        ON proposal_session_messages(session_id, message_id);
                    CREATE INDEX IF NOT EXISTS idx_proposal_session_messages_session
                        ON proposal_session_messages(session_id, id);
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

    pub fn get_recent_runs(&self, limit: usize) -> Result<Vec<RunRow>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, started_at, stopped_at, status FROM orchestration_runs ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = stmt
                .query_map(params![limit as i64], |row| {
                    Ok(RunRow {
                        id: row.get(0)?,
                        started_at: row.get(1)?,
                        stopped_at: row.get(2)?,
                        status: row.get(3)?,
                    })
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
                    AVG(duration_ms) as average_duration_ms
                 FROM change_events",
            )?;
            let (success_count, failure_count, avg_dur): (i64, i64, Option<f64>) =
                stmt.query_row([], |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?.unwrap_or(0),
                        row.get::<_, Option<i64>>(1)?.unwrap_or(0),
                        row.get::<_, Option<f64>>(2)?,
                    ))
                })?;

            let mut op_stmt = conn.prepare(
                "SELECT operation, AVG(duration_ms) FROM change_events GROUP BY operation",
            )?;
            let avg_by_op: std::collections::HashMap<String, f64> = op_stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let summary = StatsOverviewSummary {
                success_count,
                failure_count,
                in_progress_count: 0,
                average_duration_ms: avg_dur,
                average_duration_by_operation: if avg_by_op.is_empty() {
                    None
                } else {
                    Some(avg_by_op)
                },
            };

            let mut events_stmt = conn.prepare(
                "SELECT project_id, change_id, operation, success, created_at
                 FROM change_events ORDER BY id DESC LIMIT 50",
            )?;
            let recent_events: Vec<RecentEventSummary> = events_stmt
                .query_map([], |row| {
                    let success: bool = row.get::<_, i64>(3)? == 1;
                    Ok(RecentEventSummary {
                        project_id: row.get(0)?,
                        change_id: row.get(1)?,
                        operation: row.get(2)?,
                        result: if success {
                            "success".to_string()
                        } else {
                            "failure".to_string()
                        },
                        timestamp: row.get(4)?,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut proj_stmt = conn.prepare(
                "SELECT
                    project_id,
                    CASE WHEN COUNT(*) > 0
                        THEN CAST(SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) AS REAL) / COUNT(*)
                        ELSE 0.0
                    END as apply_success_rate,
                    AVG(duration_ms) as average_duration_ms,
                    SUM(CASE WHEN success = 1 THEN 1 ELSE 0 END) as success_count,
                    SUM(CASE WHEN success = 0 THEN 1 ELSE 0 END) as failure_count
                 FROM change_events GROUP BY project_id",
            )?;
            let project_stats: Vec<ProjectStatsSummary> = proj_stmt
                .query_map([], |row| {
                    Ok(ProjectStatsSummary {
                        project_id: row.get(0)?,
                        apply_success_rate: row.get(1)?,
                        average_duration_ms: row.get(2)?,
                        success_count: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
                        failure_count: row.get::<_, Option<i64>>(4)?.unwrap_or(0),
                        in_progress_count: 0,
                    })
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(StatsOverview {
                summary,
                recent_events,
                project_stats,
            })
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
                "DELETE FROM log_entries
                 WHERE created_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-' || ?1 || ' days')",
                params![days],
            )?;
            debug!(days, deleted = count, "Cleaned up old log entries");
            Ok(count)
        })
    }

    pub fn get_ui_state(&self, key: &str) -> Result<Option<String>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT value FROM ui_state WHERE key = ?1")?;
            let mut rows = stmt.query(params![key])?;
            if let Some(row) = rows.next()? {
                Ok(Some(row.get(0)?))
            } else {
                Ok(None)
            }
        })
    }

    pub fn set_ui_state(&self, key: &str, value: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO ui_state (key, value, updated_at)
                 VALUES (?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
                 ON CONFLICT(key)
                 DO UPDATE SET value = excluded.value, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
                params![key, value],
            )?;
            Ok(())
        })
    }

    pub fn delete_ui_state(&self, key: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute("DELETE FROM ui_state WHERE key = ?1", params![key])?;
            Ok(())
        })
    }

    pub fn get_all_ui_state(&self) -> Result<HashMap<String, String>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare("SELECT key, value FROM ui_state")?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows.into_iter().collect())
        })
    }

    pub fn upsert_proposal_session(&self, session: &ProposalSessionUpsert<'_>) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO proposal_sessions (
                    id, project_id, worktree_path, worktree_branch, status, created_at, updated_at, last_activity
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id)
                 DO UPDATE SET
                    project_id = excluded.project_id,
                    worktree_path = excluded.worktree_path,
                    worktree_branch = excluded.worktree_branch,
                    status = excluded.status,
                    updated_at = excluded.updated_at,
                    last_activity = excluded.last_activity",
                params![
                    session.id,
                    session.project_id,
                    session.worktree_path,
                    session.worktree_branch,
                    session.status,
                    session.created_at,
                    session.updated_at,
                    session.last_activity,
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_proposal_session_status(&self, session_id: &str, status: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE proposal_sessions
                 SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id = ?1",
                params![session_id, status],
            )?;
            Ok(())
        })
    }

    pub fn update_proposal_session_activity(
        &self,
        session_id: &str,
        activity_at: &str,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE proposal_sessions
                 SET last_activity = ?2, updated_at = ?2
                 WHERE id = ?1",
                params![session_id, activity_at],
            )?;
            Ok(())
        })
    }

    pub fn load_active_proposal_sessions(&self) -> Result<Vec<ProposalSessionDbRow>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, project_id, worktree_path, worktree_branch, status, created_at, updated_at, last_activity
                 FROM proposal_sessions
                 WHERE status IN ('active', 'timed_out', 'merging')
                 ORDER BY created_at ASC",
            )?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(ProposalSessionDbRow {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        worktree_path: row.get(2)?,
                        worktree_branch: row.get(3)?,
                        status: row.get(4)?,
                        created_at: row.get(5)?,
                        last_activity: row.get(7)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn delete_proposal_session(&self, session_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM proposal_sessions WHERE id = ?1",
                params![session_id],
            )?;
            Ok(())
        })
    }

    pub fn insert_proposal_session_message(
        &self,
        session_id: &str,
        message: &ProposalSessionMessageRecord,
    ) -> Result<()> {
        self.with_conn(|conn| {
            let hydrated = message.hydrated.unwrap_or(true);
            let is_thought = message.is_thought.unwrap_or(false);
            let tool_calls_json = message
                .tool_calls
                .as_ref()
                .and_then(|calls| serde_json::to_string(calls).ok());
            conn.execute(
                "INSERT INTO proposal_session_messages (
                    session_id, message_id, role, content, timestamp, turn_id, hydrated, is_thought, tool_calls_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(session_id, message_id)
                 DO UPDATE SET
                    role = excluded.role,
                    content = excluded.content,
                    timestamp = excluded.timestamp,
                    turn_id = excluded.turn_id,
                    hydrated = excluded.hydrated,
                    is_thought = excluded.is_thought,
                    tool_calls_json = excluded.tool_calls_json",
                params![
                    session_id,
                    message.id,
                    message.role,
                    message.content,
                    message.timestamp,
                    message.turn_id,
                    if hydrated { 1 } else { 0 },
                    if is_thought { 1 } else { 0 },
                    tool_calls_json,
                ],
            )?;
            Ok(())
        })
    }

    pub fn load_proposal_session_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<ProposalSessionMessageRecord>> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, message_id, role, content, timestamp, turn_id, hydrated, is_thought, tool_calls_json
                 FROM proposal_session_messages
                 WHERE session_id = ?1
                 ORDER BY id ASC",
            )?;
            let rows = stmt
                .query_map(params![session_id], |row| {
                    let tool_calls_json: Option<String> = row.get(8)?;
                    Ok(ProposalSessionMessageRecord {
                        id: row.get(1)?,
                        role: row.get(2)?,
                        content: row.get(3)?,
                        timestamp: row.get(4)?,
                        turn_id: row.get(5)?,
                        hydrated: row.get::<_, Option<i64>>(6)?.map(|v| v == 1),
                        is_thought: row.get::<_, Option<i64>>(7)?.map(|v| v == 1),
                        tool_calls: tool_calls_json
                            .and_then(|json| serde_json::from_str(&json).ok()),
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
    }

    pub fn delete_proposal_session_messages(&self, session_id: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "DELETE FROM proposal_session_messages WHERE session_id = ?1",
                params![session_id],
            )?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{ProposalSessionUpsert, ServerDb};
    use crate::server::proposal_session::ProposalSessionMessageRecord;

    #[test]
    fn test_server_db_init_and_run_crud() {
        let temp_dir = TempDir::new().unwrap();
        let db = ServerDb::new(temp_dir.path()).unwrap();

        let run_id = db.insert_run(Some("manual")).unwrap();
        assert!(run_id > 0);

        db.update_run_status(run_id, "success").unwrap();
        let runs = db.get_recent_runs(10).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, run_id);
        assert_eq!(runs[0].status, "success");
    }

    #[test]
    fn test_change_events_stats_and_history_queries() {
        let temp_dir = TempDir::new().unwrap();
        let db = ServerDb::new(temp_dir.path()).unwrap();

        db.insert_change_event(
            "project-a",
            "change-1",
            None,
            "apply",
            1,
            true,
            1200,
            Some(0),
            None,
            Some("ok"),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        db.insert_change_event(
            "project-a",
            "change-2",
            None,
            "archive",
            1,
            false,
            700,
            Some(1),
            Some("failed"),
            None,
            Some("err"),
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let by_change = db
            .get_events_by_project_change("project-a", "change-1")
            .unwrap();
        assert_eq!(by_change.len(), 1);
        assert_eq!(by_change[0].operation, "apply");

        let recent = db.get_recent_events("project-a", 10).unwrap();
        assert_eq!(recent.len(), 2);

        let stats = db.get_stats_overview().unwrap();
        assert_eq!(stats.summary.success_count, 1);
        assert_eq!(stats.summary.failure_count, 1);
        assert!((stats.summary.average_duration_ms.unwrap() - 950.0).abs() < f64::EPSILON);
        assert_eq!(stats.recent_events.len(), 2);
        assert_eq!(stats.project_stats.len(), 1);
    }

    #[test]
    fn test_logs_query_change_state_and_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let db = ServerDb::new(temp_dir.path()).unwrap();

        db.insert_log(
            Some("project-a"),
            "info",
            "hello",
            Some("change-1"),
            Some("apply"),
            Some(1),
        )
        .unwrap();

        let all_logs = db.query_logs(10, None, None).unwrap();
        assert_eq!(all_logs.len(), 1);
        let ts = all_logs[0].created_at.clone();

        let logs_by_project = db.query_logs(10, None, Some("project-a")).unwrap();
        assert_eq!(logs_by_project.len(), 1);

        let logs_before = db.query_logs(10, Some(&ts), None).unwrap();
        assert_eq!(logs_before.len(), 0);

        db.upsert_change_state("project-a", "change-1", false, Some("boom"))
            .unwrap();
        let states = db.load_change_states().unwrap();
        assert_eq!(states.len(), 1);
        assert!(!states[0].selected);
        assert_eq!(states[0].error_message.as_deref(), Some("boom"));

        db.delete_change_states_for_project("project-a").unwrap();
        let states = db.load_change_states().unwrap();
        assert!(states.is_empty());

        // Sleep briefly so that the log entry's created_at timestamp is strictly
        // less than SQLite's strftime('now'), which has millisecond resolution.
        // Without this, cleanup_old_logs(0) may see created_at == now and the
        // strict '<' comparison would not match.
        std::thread::sleep(std::time::Duration::from_millis(20));
        let deleted = db.cleanup_old_logs(0).unwrap();
        assert_eq!(deleted, 1);
        let remaining_logs = db.query_logs(10, None, None).unwrap();
        assert!(remaining_logs.is_empty());
    }

    #[test]
    fn test_ui_state_crud() {
        let temp_dir = TempDir::new().unwrap();
        let db = ServerDb::new(temp_dir.path()).unwrap();

        db.set_ui_state("selectedProjectId", "proj-1").unwrap();
        assert_eq!(
            db.get_ui_state("selectedProjectId").unwrap().as_deref(),
            Some("proj-1")
        );

        db.set_ui_state("selectedProjectId", "proj-2").unwrap();
        assert_eq!(
            db.get_ui_state("selectedProjectId").unwrap().as_deref(),
            Some("proj-2")
        );

        let all = db.get_all_ui_state().unwrap();
        assert_eq!(
            all.get("selectedProjectId").map(String::as_str),
            Some("proj-2")
        );

        db.delete_ui_state("selectedProjectId").unwrap();
        assert!(db.get_ui_state("selectedProjectId").unwrap().is_none());
    }

    #[test]
    fn test_proposal_session_crud() {
        let temp_dir = TempDir::new().unwrap();
        let db = ServerDb::new(temp_dir.path()).unwrap();

        let upsert = ProposalSessionUpsert {
            id: "ps-1",
            project_id: "proj-1",
            worktree_path: "/tmp/proposal-1",
            worktree_branch: "proposal/ps-1",
            status: "active",
            created_at: "2026-01-01T00:00:00Z",
            updated_at: "2026-01-01T00:00:00Z",
            last_activity: "2026-01-01T00:00:00Z",
        };
        db.upsert_proposal_session(&upsert).unwrap();

        let loaded = db.load_active_proposal_sessions().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "ps-1");
        assert_eq!(loaded[0].status, "active");

        db.update_proposal_session_status("ps-1", "timed_out")
            .unwrap();
        let loaded = db.load_active_proposal_sessions().unwrap();
        assert_eq!(loaded[0].status, "timed_out");

        db.delete_proposal_session("ps-1").unwrap();
        let loaded = db.load_active_proposal_sessions().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_proposal_session_messages_crud() {
        let temp_dir = TempDir::new().unwrap();
        let db = ServerDb::new(temp_dir.path()).unwrap();

        db.upsert_proposal_session(&ProposalSessionUpsert {
            id: "ps-1",
            project_id: "proj-1",
            worktree_path: "/tmp/proposal-1",
            worktree_branch: "proposal/ps-1",
            status: "active",
            created_at: "2026-01-01T00:00:00Z",
            updated_at: "2026-01-01T00:00:00Z",
            last_activity: "2026-01-01T00:00:00Z",
        })
        .unwrap();

        let message = ProposalSessionMessageRecord {
            id: "ps-1-user-1".to_string(),
            role: "user".to_string(),
            content: "hello".to_string(),
            timestamp: "2026-01-01T00:00:01Z".to_string(),
            turn_id: None,
            hydrated: Some(true),
            is_thought: None,
            tool_calls: None,
        };
        db.insert_proposal_session_message("ps-1", &message)
            .unwrap();

        let loaded = db.load_proposal_session_messages("ps-1").unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "ps-1-user-1");
        assert_eq!(loaded[0].content, "hello");

        db.delete_proposal_session_messages("ps-1").unwrap();
        let loaded = db.load_proposal_session_messages("ps-1").unwrap();
        assert!(loaded.is_empty());
    }
}
