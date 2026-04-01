//! Proposal session manager for the dashboard.
//!
//! Manages interactive proposal creation sessions backed by ACP stdio
//! subprocesses. Each session creates an independent worktree and one
//! `opencode acp --cwd <worktree_path>` subprocess for conversational proposal generation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::config::ProposalSessionConfig;
use crate::openspec::ProposalMetadata;
use crate::server::acp_client::{AcpClient, AcpError};
use crate::server::db::{ProposalSessionDbRow, ProposalSessionUpsert, ServerDb};
use crate::vcs::git::commands as git;

// ── Types ─────────────────────────────────────────────────────────────────

/// Status of a proposal session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProposalSessionStatus {
    /// Session is active with a running ACP subprocess.
    Active,
    /// Session is in the process of merging.
    Merging,
    /// ACP subprocess has been stopped (e.g., by inactivity timeout).
    TimedOut,
    /// Session has been closed.
    Closed,
}

impl ProposalSessionStatus {
    fn as_db_value(&self) -> &'static str {
        match self {
            ProposalSessionStatus::Active => "active",
            ProposalSessionStatus::Merging => "merging",
            ProposalSessionStatus::TimedOut => "timed_out",
            ProposalSessionStatus::Closed => "closed",
        }
    }

    fn from_db_value(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "merging" => Some(Self::Merging),
            "timed_out" => Some(Self::TimedOut),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

/// Information about a single proposal session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSessionInfo {
    pub id: String,
    pub project_id: String,
    pub worktree_path: String,
    pub worktree_branch: String,
    pub status: ProposalSessionStatus,
    pub is_dirty: bool,
    pub uncommitted_files: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_activity: String,
}

/// A detected OpenSpec change in a proposal worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedChange {
    pub id: String,
    pub title: Option<String>,
    pub metadata: ProposalMetadata,
}

/// Serialized proposal session chat message for dashboard history hydration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSessionMessageRecord {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hydrated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_thought: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ProposalSessionToolCallRecord>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSessionToolCallRecord {
    pub id: String,
    pub title: String,
    pub status: String,
}

/// Internal state of a proposal session.
#[allow(dead_code)]
pub struct ProposalSession {
    pub id: String,
    pub project_id: String,
    pub worktree_path: PathBuf,
    pub worktree_branch: String,
    pub acp_client: Arc<AcpClient>,
    pub acp_session_id: String,
    pub status: ProposalSessionStatus,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub last_db_activity_write: Option<DateTime<Utc>>,
    pub message_history: Vec<ProposalSessionMessageRecord>,
    pub active_turn_id: Option<String>,
    pub next_turn_seq: u64,
    pub next_user_seq: u64,
}

impl ProposalSession {
    /// Convert to API-facing info struct.
    pub fn to_info(&self) -> ProposalSessionInfo {
        ProposalSessionInfo {
            id: self.id.clone(),
            project_id: self.project_id.clone(),
            worktree_path: self.worktree_path.display().to_string(),
            worktree_branch: self.worktree_branch.clone(),
            status: self.status.clone(),
            is_dirty: false,
            uncommitted_files: Vec::new(),
            created_at: self.created_at.to_rfc3339(),
            updated_at: self.last_activity.to_rfc3339(),
            last_activity: self.last_activity.to_rfc3339(),
        }
    }

    /// Update the last_activity timestamp to now.
    pub fn touch(&mut self) {
        self.last_activity = Utc::now();
    }
}

// ── ProposalSessionManager ────────────────────────────────────────────────

/// Shared proposal session manager handle.
pub type SharedProposalSessionManager = Arc<RwLock<ProposalSessionManager>>;

/// Create a new shared proposal session manager.
pub fn create_proposal_session_manager(
    config: ProposalSessionConfig,
    db: Option<Arc<ServerDb>>,
) -> SharedProposalSessionManager {
    Arc::new(RwLock::new(ProposalSessionManager::new(config, db)))
}

/// Manages proposal sessions across projects.
pub struct ProposalSessionManager {
    config: ProposalSessionConfig,
    db: Option<Arc<ServerDb>>,
    /// Active sessions keyed by session ID.
    sessions: HashMap<String, ProposalSession>,
}

impl ProposalSessionManager {
    pub fn new(config: ProposalSessionConfig, db: Option<Arc<ServerDb>>) -> Self {
        Self {
            config,
            db,
            sessions: HashMap::new(),
        }
    }

    fn persist_session(&self, session: &ProposalSession) -> Result<(), ProposalSessionError> {
        if let Some(db) = &self.db {
            let created_at = session.created_at.to_rfc3339();
            let updated_at = session.last_activity.to_rfc3339();
            let payload = ProposalSessionUpsert {
                id: &session.id,
                project_id: &session.project_id,
                worktree_path: &session.worktree_path.display().to_string(),
                worktree_branch: &session.worktree_branch,
                status: session.status.as_db_value(),
                acp_session_id: &session.acp_session_id,
                created_at: &created_at,
                updated_at: &updated_at,
                last_activity: &updated_at,
            };
            db.upsert_proposal_session(&payload)
                .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?;
        }
        Ok(())
    }

    fn persist_message(
        &self,
        session_id: &str,
        message: &ProposalSessionMessageRecord,
    ) -> Result<(), ProposalSessionError> {
        if let Some(db) = &self.db {
            db.insert_proposal_session_message(session_id, message)
                .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?;
        }
        Ok(())
    }

    fn persist_activity_if_due(&mut self, session_id: &str) -> Result<(), ProposalSessionError> {
        let Some(session) = self.sessions.get_mut(session_id) else {
            return Err(ProposalSessionError::NotFound(session_id.to_string()));
        };

        let now = Utc::now();
        let should_write = session
            .last_db_activity_write
            .map(|last| (now - last).num_seconds() >= 60)
            .unwrap_or(true);
        if !should_write {
            return Ok(());
        }

        if let Some(db) = &self.db {
            let ts = now.to_rfc3339();
            db.update_proposal_session_activity(&session.id, &ts)
                .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?;
            session.last_db_activity_write = Some(now);
        }

        Ok(())
    }

    /// Create a new proposal session for a project.
    ///
    /// This will:
    /// 1. Create a new worktree on branch `proposal/<session_id>`
    /// 2. Spawn an ACP subprocess in the worktree directory
    /// 3. Create an ACP session via JSON-RPC
    pub async fn create_session(
        &mut self,
        project_id: &str,
        repo_root: &Path,
    ) -> Result<ProposalSessionInfo, ProposalSessionError> {
        let session_id = generate_session_id();
        let branch_name = format!("proposal/{}", session_id);

        info!(
            session_id = %session_id,
            project_id = %project_id,
            branch = %branch_name,
            "Creating proposal session"
        );

        // Get HEAD commit for worktree creation
        let head_commit = git::get_current_commit(repo_root)
            .await
            .map_err(|e| ProposalSessionError::Git(format!("Failed to get HEAD: {}", e)))?;

        // Determine worktree path
        let worktree_path = repo_root
            .parent()
            .unwrap_or(repo_root)
            .join(format!("proposal-{}", &session_id));

        // Create worktree
        let worktree_path_str = worktree_path
            .to_str()
            .ok_or_else(|| ProposalSessionError::Git("Invalid worktree path".into()))?
            .to_string();
        git::worktree_add(repo_root, &worktree_path_str, &branch_name, &head_commit)
            .await
            .map_err(|e| ProposalSessionError::Git(format!("Failed to create worktree: {}", e)))?;

        info!(
            worktree = %worktree_path.display(),
            branch = %branch_name,
            "Worktree created for proposal session"
        );

        // Spawn ACP subprocess with explicit --cwd for the proposal worktree.
        let mut acp_config = self.config.clone();
        let mut transport_args = acp_config.transport_args.clone();
        if transport_args.is_empty() {
            transport_args.push("acp".to_string());
        }
        if !transport_args.iter().any(|arg| arg == "--cwd") {
            transport_args.push("--cwd".to_string());
            transport_args.push(worktree_path.display().to_string());
        }
        acp_config.transport_args = transport_args;

        let acp_client = AcpClient::spawn(&acp_config, &worktree_path)
            .await
            .map_err(ProposalSessionError::Acp)?;

        acp_client
            .initialize()
            .await
            .map_err(ProposalSessionError::Acp)?;

        let acp_session_id = acp_client
            .create_session()
            .await
            .map_err(ProposalSessionError::Acp)?;

        let now = Utc::now();
        let session = ProposalSession {
            id: session_id.clone(),
            project_id: project_id.to_string(),
            worktree_path: worktree_path.clone(),
            worktree_branch: branch_name.clone(),
            acp_client,
            acp_session_id,
            status: ProposalSessionStatus::Active,
            created_at: now,
            last_activity: now,
            last_db_activity_write: None,
            message_history: Vec::new(),
            active_turn_id: None,
            next_turn_seq: 0,
            next_user_seq: 0,
        };

        self.persist_session(&session)?;

        let info = session.to_info();
        self.sessions.insert(session_id, session);

        Ok(info)
    }

    /// List all active sessions for a project.
    pub fn list_sessions(&self, project_id: &str) -> Vec<ProposalSessionInfo> {
        self.sessions
            .values()
            .filter(|s| s.project_id == project_id)
            .map(|s| s.to_info())
            .collect()
    }

    pub async fn restore_session(
        &mut self,
        row: &ProposalSessionDbRow,
    ) -> Result<Option<ProposalSessionInfo>, ProposalSessionError> {
        let worktree_path = PathBuf::from(&row.worktree_path);
        if !worktree_path.exists() {
            if let Some(db) = &self.db {
                db.delete_proposal_session_messages(&row.id)
                    .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?;
                db.delete_proposal_session(&row.id)
                    .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?;
            }
            return Ok(None);
        }

        let mut status = ProposalSessionStatus::from_db_value(&row.status)
            .unwrap_or(ProposalSessionStatus::Active);
        if status == ProposalSessionStatus::TimedOut {
            status = ProposalSessionStatus::Active;
        }

        let mut acp_config = self.config.clone();
        let mut transport_args = acp_config.transport_args.clone();
        if transport_args.is_empty() {
            transport_args.push("acp".to_string());
        }
        if !transport_args.iter().any(|arg| arg == "--cwd") {
            transport_args.push("--cwd".to_string());
            transport_args.push(worktree_path.display().to_string());
        }
        acp_config.transport_args = transport_args;

        let acp_client = AcpClient::spawn(&acp_config, &worktree_path)
            .await
            .map_err(ProposalSessionError::Acp)?;
        acp_client
            .initialize()
            .await
            .map_err(ProposalSessionError::Acp)?;
        let acp_session_id = acp_client
            .create_session()
            .await
            .map_err(ProposalSessionError::Acp)?;

        let message_history = if let Some(db) = &self.db {
            db.load_proposal_session_messages(&row.id)
                .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?
        } else {
            Vec::new()
        };

        let created_at = chrono::DateTime::parse_from_rfc3339(&row.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let last_activity = chrono::DateTime::parse_from_rfc3339(&row.last_activity)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let mut next_turn_seq = 0_u64;
        let mut next_user_seq = 0_u64;
        for message in &message_history {
            if let Some(turn_id) = &message.turn_id {
                if let Some(num) = turn_id
                    .rsplit('-')
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                {
                    next_turn_seq = next_turn_seq.max(num);
                }
            }
            if message.role == "user" {
                if let Some(num) = message
                    .id
                    .rsplit('-')
                    .next()
                    .and_then(|v| v.parse::<u64>().ok())
                {
                    next_user_seq = next_user_seq.max(num);
                }
            }
        }

        let session = ProposalSession {
            id: row.id.clone(),
            project_id: row.project_id.clone(),
            worktree_path,
            worktree_branch: row.worktree_branch.clone(),
            acp_client,
            acp_session_id,
            status,
            created_at,
            last_activity,
            last_db_activity_write: Some(last_activity),
            message_history,
            active_turn_id: None,
            next_turn_seq,
            next_user_seq,
        };

        self.persist_session(&session)?;

        let info = session.to_info();
        self.sessions.insert(row.id.clone(), session);
        Ok(Some(info))
    }

    /// Get a session by ID.
    pub fn get_session(&self, session_id: &str) -> Option<&ProposalSession> {
        self.sessions.get(session_id)
    }

    pub fn touch_session_activity(&mut self, session_id: &str) -> Result<(), ProposalSessionError> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            session.touch();
        } else {
            return Err(ProposalSessionError::NotFound(session_id.to_string()));
        }

        self.persist_activity_if_due(session_id)
    }

    /// Return serialized chat messages for a proposal session.
    #[allow(dead_code)]
    pub fn list_messages(
        &self,
        session_id: &str,
    ) -> Result<Vec<ProposalSessionMessageRecord>, ProposalSessionError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;
        Ok(session.message_history.clone())
    }

    /// Record an outgoing user prompt for history hydration.
    #[allow(dead_code)]
    pub fn record_user_prompt(
        &mut self,
        session_id: &str,
        content: &str,
    ) -> Result<ProposalSessionMessageRecord, ProposalSessionError> {
        self.record_user_prompt_with_client_message_id(session_id, content, None)
    }

    /// Record an outgoing user prompt and preserve client_message_id for reconnect dedupe.
    pub fn record_user_prompt_with_client_message_id(
        &mut self,
        session_id: &str,
        content: &str,
        client_message_id: Option<&str>,
    ) -> Result<ProposalSessionMessageRecord, ProposalSessionError> {
        let message = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;
            session.next_user_seq += 1;
            let now = Utc::now().to_rfc3339();
            let message = ProposalSessionMessageRecord {
                id: format!("{}-user-{}", session.id, session.next_user_seq),
                role: "user".to_string(),
                content: content.to_string(),
                timestamp: now,
                turn_id: None,
                client_message_id: client_message_id.map(|id| id.to_string()),
                hydrated: Some(true),
                is_thought: None,
                tool_calls: None,
            };
            session.message_history.push(message.clone());
            message
        };

        self.persist_message(session_id, &message)?;
        Ok(message)
    }

    pub fn is_client_message_recorded(
        &self,
        session_id: &str,
        client_message_id: &str,
    ) -> Result<bool, ProposalSessionError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        Ok(session
            .message_history
            .iter()
            .any(|message| message.client_message_id.as_deref() == Some(client_message_id)))
    }

    pub fn get_active_turn_id(
        &self,
        session_id: &str,
    ) -> Result<Option<String>, ProposalSessionError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;
        Ok(session.active_turn_id.clone())
    }

    /// Append an assistant text chunk to the active turn in message history.
    #[allow(dead_code)]
    pub fn append_assistant_chunk(
        &mut self,
        session_id: &str,
        chunk: &str,
    ) -> Result<String, ProposalSessionError> {
        self.append_assistant_chunk_with_kind(session_id, chunk, false)
    }

    /// Append an assistant thought chunk to the active turn in message history.
    #[allow(dead_code)]
    pub fn append_assistant_thought_chunk(
        &mut self,
        session_id: &str,
        chunk: &str,
    ) -> Result<String, ProposalSessionError> {
        self.append_assistant_chunk_with_kind(session_id, chunk, true)
    }

    fn append_assistant_chunk_with_kind(
        &mut self,
        session_id: &str,
        chunk: &str,
        is_thought: bool,
    ) -> Result<String, ProposalSessionError> {
        let (turn_id, maybe_message) = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

            let turn_id = if let Some(turn_id) = session.active_turn_id.clone() {
                turn_id
            } else {
                session.next_turn_seq += 1;
                let turn_id = format!("{}-turn-{}", session.id, session.next_turn_seq);
                let now = Utc::now().to_rfc3339();
                session.message_history.push(ProposalSessionMessageRecord {
                    id: format!("assistant-{}", turn_id),
                    role: "assistant".to_string(),
                    content: String::new(),
                    timestamp: now,
                    turn_id: Some(turn_id.clone()),
                    client_message_id: None,
                    hydrated: Some(true),
                    is_thought: if is_thought { Some(true) } else { None },
                    tool_calls: None,
                });
                session.active_turn_id = Some(turn_id.clone());
                turn_id
            };

            let updated = if let Some(message) = session
                .message_history
                .iter_mut()
                .rev()
                .find(|message| message.turn_id.as_deref() == Some(turn_id.as_str()))
            {
                message.content.push_str(chunk);
                if is_thought {
                    message.is_thought = Some(true);
                }
                Some(message.clone())
            } else {
                None
            };

            (turn_id, updated)
        };

        let _ = maybe_message;
        Ok(turn_id)
    }

    /// Record a tool call event into the currently active assistant turn.
    #[allow(dead_code)]
    pub fn record_tool_call(
        &mut self,
        session_id: &str,
        tool_call_id: &str,
        title: &str,
        status: &str,
    ) -> Result<(), ProposalSessionError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        let turn_id = if let Some(turn_id) = session.active_turn_id.clone() {
            turn_id
        } else {
            session.next_turn_seq += 1;
            let turn_id = format!("{}-turn-{}", session.id, session.next_turn_seq);
            let now = Utc::now().to_rfc3339();
            session.message_history.push(ProposalSessionMessageRecord {
                id: format!("assistant-{}", turn_id),
                role: "assistant".to_string(),
                content: String::new(),
                timestamp: now,
                turn_id: Some(turn_id.clone()),
                client_message_id: None,
                hydrated: Some(true),
                is_thought: None,
                tool_calls: Some(Vec::new()),
            });
            session.active_turn_id = Some(turn_id.clone());
            turn_id
        };

        if let Some(message) = session
            .message_history
            .iter_mut()
            .rev()
            .find(|message| message.turn_id.as_deref() == Some(turn_id.as_str()))
        {
            let tool_calls = message.tool_calls.get_or_insert_with(Vec::new);
            if let Some(existing) = tool_calls.iter_mut().find(|call| call.id == tool_call_id) {
                existing.status = status.to_string();
                if !title.is_empty() {
                    existing.title = title.to_string();
                }
            } else {
                tool_calls.push(ProposalSessionToolCallRecord {
                    id: tool_call_id.to_string(),
                    title: title.to_string(),
                    status: status.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Update a tool call status in message history.
    #[allow(dead_code)]
    pub fn update_tool_call_status(
        &mut self,
        session_id: &str,
        tool_call_id: &str,
        status: &str,
    ) -> Result<(), ProposalSessionError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        for message in session.message_history.iter_mut().rev() {
            if let Some(tool_calls) = message.tool_calls.as_mut() {
                if let Some(existing) = tool_calls.iter_mut().find(|call| call.id == tool_call_id) {
                    existing.status = status.to_string();
                    break;
                }
            }
        }

        Ok(())
    }

    /// Mark the active assistant turn complete.
    #[allow(dead_code)]
    pub fn complete_active_turn(
        &mut self,
        session_id: &str,
    ) -> Result<Option<String>, ProposalSessionError> {
        let (completed_turn_id, maybe_message) = {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;
            let active_turn_id = session.active_turn_id.clone();
            session.active_turn_id = None;
            let maybe_message = active_turn_id.clone().and_then(|turn_id| {
                session
                    .message_history
                    .iter()
                    .rev()
                    .find(|m| m.turn_id.as_deref() == Some(turn_id.as_str()))
                    .cloned()
            });
            (active_turn_id, maybe_message)
        };

        if let Some(message) = maybe_message {
            self.persist_message(session_id, &message)?;
        }
        Ok(completed_turn_id)
    }

    /// Check if a session's worktree has uncommitted changes.
    pub async fn check_dirty(
        &self,
        session_id: &str,
    ) -> Result<(bool, Vec<String>), ProposalSessionError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        let (has_changes, status_output) = git::has_uncommitted_changes(&session.worktree_path)
            .await
            .map_err(|e| {
                ProposalSessionError::Git(format!("Failed to check dirty state: {}", e))
            })?;

        let files: Vec<String> = if has_changes {
            status_output
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        } else {
            Vec::new()
        };

        Ok((has_changes, files))
    }

    /// Close (delete) a proposal session.
    ///
    /// If `force` is false and the worktree is dirty, returns an error with the
    /// list of uncommitted files.
    pub async fn close_session(
        &mut self,
        session_id: &str,
        force: bool,
        repo_root: &Path,
    ) -> Result<(), ProposalSessionError> {
        // Check dirty state if not forcing
        if !force {
            let (is_dirty, files) = self.check_dirty(session_id).await?;
            if is_dirty {
                return Err(ProposalSessionError::DirtyWorktree { files });
            }
        }

        let session = self
            .sessions
            .remove(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        info!(
            session_id = %session_id,
            force = %force,
            "Closing proposal session"
        );

        // Kill ACP process
        session.acp_client.kill().await;

        // Remove worktree
        let wt_path_str = session.worktree_path.to_string_lossy().to_string();
        if let Err(e) = git::worktree_remove(repo_root, &wt_path_str).await {
            warn!(
                error = %e,
                worktree = %session.worktree_path.display(),
                "Failed to remove worktree (may already be removed)"
            );
        }

        // Delete the branch
        if let Err(e) = git::branch_delete(repo_root, &session.worktree_branch).await {
            debug!(
                error = %e,
                branch = %session.worktree_branch,
                "Failed to delete proposal branch"
            );
        }

        if let Some(db) = &self.db {
            db.delete_proposal_session_messages(&session.id)
                .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?;
            db.delete_proposal_session(&session.id)
                .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?;
        }

        Ok(())
    }

    /// Merge a proposal session's worktree into the project base branch.
    pub async fn merge_session(
        &mut self,
        session_id: &str,
        repo_root: &Path,
        base_branch: &str,
    ) -> Result<(), ProposalSessionError> {
        // Check dirty state first
        let (is_dirty, files) = self.check_dirty(session_id).await?;
        if is_dirty {
            return Err(ProposalSessionError::DirtyWorktree { files });
        }

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        session.status = ProposalSessionStatus::Merging;
        let worktree_branch = session.worktree_branch.clone();

        info!(
            session_id = %session_id,
            branch = %worktree_branch,
            base = %base_branch,
            "Merging proposal session"
        );

        // Merge the proposal branch into the base branch
        git::merge_branch(repo_root, &worktree_branch)
            .await
            .map_err(|e| ProposalSessionError::MergeConflict(format!("{}", e)))?;

        // Now close the session (force=true since we just merged)
        // Remove from sessions map
        let session = self
            .sessions
            .remove(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        // Kill ACP process
        session.acp_client.kill().await;

        // Remove worktree
        let wt_path_str = session.worktree_path.to_string_lossy().to_string();
        if let Err(e) = git::worktree_remove(repo_root, &wt_path_str).await {
            warn!(
                error = %e,
                worktree = %session.worktree_path.display(),
                "Failed to remove worktree after merge"
            );
        }

        // Delete the branch
        if let Err(e) = git::branch_delete(repo_root, &worktree_branch).await {
            debug!(
                error = %e,
                branch = %worktree_branch,
                "Failed to delete proposal branch after merge"
            );
        }

        if let Some(db) = &self.db {
            db.delete_proposal_session_messages(&session.id)
                .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?;
            db.delete_proposal_session(&session.id)
                .map_err(|e| ProposalSessionError::Persistence(e.to_string()))?;
        }

        Ok(())
    }

    /// Detect OpenSpec changes in a session's worktree.
    pub async fn detect_changes(
        &self,
        session_id: &str,
    ) -> Result<Vec<DetectedChange>, ProposalSessionError> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or(ProposalSessionError::NotFound(session_id.to_string()))?;

        let changes_dir = session.worktree_path.join("openspec").join("changes");
        let mut detected = Vec::new();

        if !changes_dir.exists() {
            return Ok(detected);
        }

        let entries = std::fs::read_dir(&changes_dir).map_err(|e| {
            ProposalSessionError::Git(format!("Failed to read changes directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                ProposalSessionError::Git(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Skip archive directory
            if path.file_name().and_then(|n| n.to_str()) == Some("archive") {
                continue;
            }

            let proposal_path = path.join("proposal.md");
            if !proposal_path.exists() {
                continue;
            }

            let change_id = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Try to extract title and metadata from proposal.md
            let title = extract_proposal_title(&proposal_path);
            let metadata = crate::openspec::parse_proposal_metadata_from_file(&proposal_path);

            detected.push(DetectedChange {
                id: change_id,
                title,
                metadata,
            });
        }

        Ok(detected)
    }

    /// Scan for sessions that have exceeded the inactivity timeout and stop their ACP processes.
    pub async fn scan_timeouts(&mut self) {
        let timeout_secs = self.config.session_inactivity_timeout_secs;
        if timeout_secs == 0 {
            return;
        }

        let now = Utc::now();
        let mut timed_out = Vec::new();

        for (id, session) in &self.sessions {
            if session.status != ProposalSessionStatus::Active {
                continue;
            }
            let elapsed = (now - session.last_activity).num_seconds();
            if elapsed > timeout_secs as i64 {
                timed_out.push(id.clone());
            }
        }

        for id in timed_out {
            if let Some(session) = self.sessions.get_mut(&id) {
                info!(
                    session_id = %id,
                    "Proposal session timed out, stopping ACP subprocess"
                );
                session.acp_client.kill().await;
                session.status = ProposalSessionStatus::TimedOut;
                if let Some(db) = &self.db {
                    if let Err(e) = db.update_proposal_session_status(
                        &id,
                        ProposalSessionStatus::TimedOut.as_db_value(),
                    ) {
                        warn!(
                            session_id = %id,
                            error = %e,
                            "Failed to persist timed-out proposal session status"
                        );
                    }
                }
            }
        }
    }

    /// Kill all ACP processes and remove clean worktrees (shutdown cleanup).
    pub async fn cleanup_all(&mut self, repo_root: Option<&Path>) {
        let session_ids: Vec<String> = self.sessions.keys().cloned().collect();

        for id in session_ids {
            if let Some(session) = self.sessions.remove(&id) {
                info!(session_id = %id, "Cleaning up proposal session");
                session.acp_client.kill().await;

                if let Some(root) = repo_root {
                    // Only remove clean worktrees
                    let is_dirty = git::has_uncommitted_changes(&session.worktree_path)
                        .await
                        .map(|(has_changes, _)| has_changes)
                        .unwrap_or(true);

                    if !is_dirty {
                        let wt_path_str = session.worktree_path.to_string_lossy().to_string();
                        if let Err(e) = git::worktree_remove(root, &wt_path_str).await {
                            warn!(
                                error = %e,
                                worktree = %session.worktree_path.display(),
                                "Failed to remove worktree during cleanup"
                            );
                        }
                    } else {
                        info!(
                            worktree = %session.worktree_path.display(),
                            "Preserving dirty worktree during cleanup"
                        );
                    }
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

/// Generate a unique session ID.
fn generate_session_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let id: u64 = rng.gen();
    format!("ps-{:016x}", id)
}

/// Extract the title from a proposal.md file (first `# ` heading).
fn extract_proposal_title(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(title) = trimmed.strip_prefix("# ") {
            // Strip common prefixes like "Change: "
            let title = title.strip_prefix("Change: ").unwrap_or(title);
            return Some(title.trim().to_string());
        }
    }
    None
}

// ── Error types ───────────────────────────────────────────────────────────

/// Errors from proposal session operations.
#[derive(Debug, thiserror::Error)]
pub enum ProposalSessionError {
    #[error("Proposal session not found: {0}")]
    NotFound(String),

    #[error("Git operation failed: {0}")]
    Git(String),

    #[error("ACP transport error: {0}")]
    Acp(#[from] AcpError),

    #[error("Worktree has uncommitted changes")]
    DirtyWorktree { files: Vec<String> },

    #[error("Merge conflict: {0}")]
    MergeConflict(String),

    #[error("Persistence error: {0}")]
    Persistence(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_id() {
        let id1 = generate_session_id();
        let id2 = generate_session_id();
        assert!(id1.starts_with("ps-"));
        assert_ne!(id1, id2);
        // ps- prefix + 16 hex chars
        assert_eq!(id1.len(), 3 + 16);
    }

    #[test]
    fn test_extract_proposal_title() {
        let dir = tempfile::TempDir::new().unwrap();
        let proposal = dir.path().join("proposal.md");

        std::fs::write(&proposal, "# Change: My Feature\n\n## Problem\nSomething\n").unwrap();

        let title = extract_proposal_title(&proposal);
        assert_eq!(title, Some("My Feature".to_string()));
    }

    #[test]
    fn test_extract_proposal_title_no_change_prefix() {
        let dir = tempfile::TempDir::new().unwrap();
        let proposal = dir.path().join("proposal.md");

        std::fs::write(&proposal, "# Add authentication\n\n## Why\nBecause\n").unwrap();

        let title = extract_proposal_title(&proposal);
        assert_eq!(title, Some("Add authentication".to_string()));
    }

    #[test]
    fn test_detected_change_metadata_serializes() {
        let change = DetectedChange {
            id: "add-auth".to_string(),
            title: Some("Add authentication".to_string()),
            metadata: ProposalMetadata {
                change_type: Some("implementation".to_string()),
                priority: Some(crate::openspec::ProposalPriority::High),
                dependencies: vec!["base-change".to_string()],
                references: vec!["src/demo.py".to_string()],
                warnings: vec![],
            },
        };

        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["metadata"]["priority"], "high");
        assert_eq!(json["metadata"]["dependencies"][0], "base-change");
        assert_eq!(json["metadata"]["references"][0], "src/demo.py");
    }

    #[test]
    fn test_extract_proposal_title_missing_file() {
        let title = extract_proposal_title(Path::new("/nonexistent/proposal.md"));
        assert!(title.is_none());
    }

    #[test]
    fn test_proposal_session_info_serialization() {
        let info = ProposalSessionInfo {
            id: "ps-abc123".to_string(),
            project_id: "proj1".to_string(),
            worktree_path: "/tmp/proposal-abc123".to_string(),
            worktree_branch: "proposal/ps-abc123".to_string(),
            status: ProposalSessionStatus::Active,
            is_dirty: false,
            uncommitted_files: Vec::new(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            last_activity: "2025-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["status"], "active");
        assert_eq!(json["id"], "ps-abc123");
    }

    #[test]
    fn test_detected_change_serialization() {
        let change = DetectedChange {
            id: "add-auth".to_string(),
            title: Some("Add authentication".to_string()),
            metadata: ProposalMetadata::default(),
        };
        let json = serde_json::to_value(&change).unwrap();
        assert_eq!(json["id"], "add-auth");
        assert_eq!(json["title"], "Add authentication");
    }

    #[test]
    fn test_proposal_session_manager_new() {
        let config = ProposalSessionConfig::default();
        let manager = ProposalSessionManager::new(config, None);
        assert!(manager.sessions.is_empty());
    }

    #[test]
    fn test_proposal_session_manager_list_empty() {
        let config = ProposalSessionConfig::default();
        let manager = ProposalSessionManager::new(config, None);
        let sessions = manager.list_sessions("proj1");
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_append_assistant_thought_chunk_sets_is_thought() {
        let config = ProposalSessionConfig::default();
        let mut manager = ProposalSessionManager::new(config, None);

        let session_id = "ps-test".to_string();
        manager.sessions.insert(
            session_id.clone(),
            ProposalSession {
                id: session_id.clone(),
                project_id: "proj1".to_string(),
                worktree_path: PathBuf::from("/tmp/proposal-ps-test"),
                worktree_branch: "proposal/ps-test".to_string(),
                acp_client: Arc::new(AcpClient::new_for_test()),
                acp_session_id: "acp-session-1".to_string(),
                status: ProposalSessionStatus::Active,
                created_at: Utc::now(),
                last_activity: Utc::now(),
                last_db_activity_write: None,
                message_history: Vec::new(),
                active_turn_id: None,
                next_turn_seq: 0,
                next_user_seq: 0,
            },
        );

        let turn_id = manager
            .append_assistant_thought_chunk(&session_id, "thinking")
            .expect("append thought chunk should succeed");
        assert_eq!(turn_id, "ps-test-turn-1");

        let messages = manager
            .list_messages(&session_id)
            .expect("list_messages should succeed");
        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.role, "assistant");
        assert_eq!(message.content, "thinking");
        assert_eq!(message.is_thought, Some(true));
    }
}
