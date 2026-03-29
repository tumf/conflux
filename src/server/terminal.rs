//! Terminal session management for the dashboard.
//!
//! Provides PTY-backed interactive terminal sessions that can be attached
//! to via WebSocket connections from the dashboard frontend.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use tracing::{debug, info};

/// Unique identifier for a terminal session.
pub type SessionId = String;

/// Default terminal dimensions.
const DEFAULT_ROWS: u16 = 24;
const DEFAULT_COLS: u16 = 80;

/// Maximum number of bytes to buffer for output broadcast.
const OUTPUT_CHANNEL_CAPACITY: usize = 256;

/// Information about a terminal session visible to the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalSessionInfo {
    pub id: SessionId,
    pub cwd: String,
    pub rows: u16,
    pub cols: u16,
    pub created_at: String,
}

/// Request to create a new terminal session.
#[derive(Debug, Deserialize)]
pub struct CreateTerminalRequest {
    /// Working directory for the terminal session (resolved server-side if project_id + root are provided).
    pub cwd: String,
    /// Optional initial rows (default: 24).
    pub rows: Option<u16>,
    /// Optional initial cols (default: 80).
    pub cols: Option<u16>,
}

/// Request from the dashboard to create a terminal session with project context.
/// The server resolves the cwd from project_id and root.
#[derive(Debug, Deserialize)]
pub struct CreateTerminalFromContextRequest {
    /// Project identifier.
    pub project_id: String,
    /// Root parameter matching file browser context: "base" or "worktree:<branch>".
    pub root: String,
    /// Optional initial rows (default: 24).
    pub rows: Option<u16>,
    /// Optional initial cols (default: 80).
    pub cols: Option<u16>,
}

/// Request to resize a terminal session.
#[derive(Debug, Deserialize)]
pub struct ResizeTerminalRequest {
    pub rows: u16,
    pub cols: u16,
}

/// Command sent to the PTY management thread for resize operations.
enum PtyCommand {
    Resize { rows: u16, cols: u16 },
    Shutdown,
}

/// Internal representation of a running terminal session.
struct TerminalSession {
    info: TerminalSessionInfo,
    /// Writer to send input to the PTY.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    /// Broadcast sender for terminal output.
    output_tx: broadcast::Sender<Vec<u8>>,
    /// Channel to send commands (resize) to the PTY management thread.
    pty_cmd_tx: mpsc::UnboundedSender<PtyCommand>,
}

/// Thread-safe terminal session manager.
pub struct TerminalManager {
    sessions: RwLock<HashMap<SessionId, TerminalSession>>,
}

/// Shared terminal manager type.
pub type SharedTerminalManager = Arc<TerminalManager>;

/// Create a new shared terminal manager.
pub fn create_terminal_manager() -> SharedTerminalManager {
    Arc::new(TerminalManager {
        sessions: RwLock::new(HashMap::new()),
    })
}

impl TerminalManager {
    /// Create a new terminal session with the given working directory.
    pub async fn create_session(
        &self,
        request: CreateTerminalRequest,
    ) -> Result<TerminalSessionInfo, String> {
        let rows = request.rows.unwrap_or(DEFAULT_ROWS);
        let cols = request.cols.unwrap_or(DEFAULT_COLS);
        let cwd_path = PathBuf::from(&request.cwd);

        if !cwd_path.exists() {
            return Err(format!("Working directory does not exist: {}", request.cwd));
        }

        let session_id = generate_session_id();
        let created_at = chrono::Utc::now().to_rfc3339();

        info!(
            session_id = %session_id,
            cwd = %request.cwd,
            rows = rows,
            cols = cols,
            "Creating terminal session"
        );

        let cwd = request.cwd.clone();
        let sid = session_id.clone();
        let ts = created_at.clone();

        // Channel for PTY commands (resize, shutdown)
        let (pty_cmd_tx, pty_cmd_rx) = mpsc::unbounded_channel::<PtyCommand>();
        let (output_tx, _) = broadcast::channel(OUTPUT_CHANNEL_CAPACITY);

        // Spawn PTY in a blocking thread since portable-pty is synchronous.
        // The master PTY stays in this thread and is controlled via pty_cmd_rx.
        let output_tx_clone = output_tx.clone();
        let sid_clone = sid.clone();

        let (writer_tx, writer_rx) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            let pty_system = native_pty_system();
            let pair = match pty_system.openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            }) {
                Ok(pair) => pair,
                Err(e) => {
                    let _ = writer_tx.send(Err(format!("Failed to open PTY: {}", e)));
                    return;
                }
            };

            // Build shell command
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            let mut cmd = CommandBuilder::new(&shell);
            cmd.arg("-l"); // login shell
            cmd.cwd(&cwd);

            // Spawn the shell
            let _child = match pair.slave.spawn_command(cmd) {
                Ok(child) => child,
                Err(e) => {
                    let _ = writer_tx.send(Err(format!("Failed to spawn shell: {}", e)));
                    return;
                }
            };

            // Drop slave - consumed by child process
            drop(pair.slave);

            // Get reader and writer from master
            let reader = match pair.master.try_clone_reader() {
                Ok(r) => r,
                Err(e) => {
                    let _ = writer_tx.send(Err(format!("Failed to clone PTY reader: {}", e)));
                    return;
                }
            };
            let writer = match pair.master.take_writer() {
                Ok(w) => w,
                Err(e) => {
                    let _ = writer_tx.send(Err(format!("Failed to take PTY writer: {}", e)));
                    return;
                }
            };

            // Send the writer back to the async world
            let _ = writer_tx.send(Ok(writer));

            // Spawn a reader thread
            let tx = output_tx_clone;
            let reader_sid = sid_clone.clone();
            std::thread::spawn(move || {
                read_pty_output(reader, tx, reader_sid);
            });

            // This thread now handles PTY commands (resize) since master stays here
            let mut pty_cmd_rx = pty_cmd_rx;
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                while let Some(cmd) = pty_cmd_rx.recv().await {
                    match cmd {
                        PtyCommand::Resize { rows, cols } => {
                            if let Err(e) = pair.master.resize(PtySize {
                                rows,
                                cols,
                                pixel_width: 0,
                                pixel_height: 0,
                            }) {
                                debug!(session_id = %sid_clone, error = %e, "Failed to resize PTY");
                            }
                        }
                        PtyCommand::Shutdown => {
                            break;
                        }
                    }
                }
            });

            info!(session_id = %sid_clone, "PTY management thread exiting");
            // master is dropped here, killing the child process
        });

        // Wait for the writer to be sent back
        let writer = writer_rx
            .await
            .map_err(|_| "PTY thread terminated before sending writer".to_string())??;

        let info = TerminalSessionInfo {
            id: sid.clone(),
            cwd: request.cwd,
            rows,
            cols,
            created_at: ts,
        };

        let session = TerminalSession {
            info: info.clone(),
            writer: Arc::new(Mutex::new(writer)),
            output_tx,
            pty_cmd_tx,
        };

        self.sessions.write().await.insert(session_id, session);
        Ok(info)
    }

    /// List all active terminal sessions.
    pub async fn list_sessions(&self) -> Vec<TerminalSessionInfo> {
        let sessions = self.sessions.read().await;
        sessions.values().map(|s| s.info.clone()).collect()
    }

    /// Delete a terminal session, killing the underlying shell.
    pub async fn delete_session(&self, session_id: &str) -> Result<(), String> {
        let session = self
            .sessions
            .write()
            .await
            .remove(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        info!(session_id = %session_id, "Deleting terminal session");

        // Send shutdown command to the PTY management thread
        let _ = session.pty_cmd_tx.send(PtyCommand::Shutdown);
        // Dropping session drops writer + broadcast sender
        drop(session);
        Ok(())
    }

    /// Write input to a terminal session.
    pub async fn write_input(&self, session_id: &str, data: &[u8]) -> Result<(), String> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        let mut writer = session.writer.lock().await;
        writer
            .write_all(data)
            .map_err(|e| format!("Failed to write to PTY: {}", e))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush PTY: {}", e))?;
        Ok(())
    }

    /// Subscribe to output from a terminal session.
    pub async fn subscribe_output(
        &self,
        session_id: &str,
    ) -> Result<broadcast::Receiver<Vec<u8>>, String> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        Ok(session.output_tx.subscribe())
    }

    /// Resize a terminal session.
    pub async fn resize_session(
        &self,
        session_id: &str,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;

        debug!(session_id = %session_id, rows = rows, cols = cols, "Resizing terminal");

        session
            .pty_cmd_tx
            .send(PtyCommand::Resize { rows, cols })
            .map_err(|_| "PTY management thread is gone".to_string())?;

        session.info.rows = rows;
        session.info.cols = cols;
        Ok(())
    }

    /// Check if a session exists.
    pub async fn session_exists(&self, session_id: &str) -> bool {
        self.sessions.read().await.contains_key(session_id)
    }
}

/// Read PTY output in a blocking thread and broadcast to subscribers.
fn read_pty_output(
    mut reader: Box<dyn Read + Send>,
    tx: broadcast::Sender<Vec<u8>>,
    session_id: String,
) {
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                debug!(session_id = %session_id, "PTY EOF");
                break;
            }
            Ok(n) => {
                let data = buf[..n].to_vec();
                // Ignore send error - it just means no subscribers
                let _ = tx.send(data);
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    continue;
                }
                // On other errors (e.g., PTY closed), stop reading
                debug!(session_id = %session_id, error = %e, "PTY read error");
                break;
            }
        }
    }

    info!(session_id = %session_id, "PTY reader thread exiting");
}

/// Generate a random session ID.
fn generate_session_id() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let id: u64 = rng.gen();
    format!("term-{:016x}", id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_list_sessions() {
        let manager = create_terminal_manager();

        // Create a session in /tmp (guaranteed to exist)
        let info = manager
            .create_session(CreateTerminalRequest {
                cwd: "/tmp".to_string(),
                rows: Some(24),
                cols: Some(80),
            })
            .await
            .unwrap();

        assert!(info.id.starts_with("term-"));
        assert_eq!(info.cwd, "/tmp");
        assert_eq!(info.rows, 24);
        assert_eq!(info.cols, 80);

        // List sessions
        let sessions = manager.list_sessions().await;
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, info.id);

        // Cleanup
        manager.delete_session(&info.id).await.unwrap();
        let sessions = manager.list_sessions().await;
        assert_eq!(sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_create_session_invalid_cwd() {
        let manager = create_terminal_manager();
        let result = manager
            .create_session(CreateTerminalRequest {
                cwd: "/nonexistent/path/that/does/not/exist".to_string(),
                rows: None,
                cols: None,
            })
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_delete_nonexistent_session() {
        let manager = create_terminal_manager();
        let result = manager.delete_session("nonexistent").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_session_exists() {
        let manager = create_terminal_manager();

        assert!(!manager.session_exists("nonexistent").await);

        let info = manager
            .create_session(CreateTerminalRequest {
                cwd: "/tmp".to_string(),
                rows: None,
                cols: None,
            })
            .await
            .unwrap();

        assert!(manager.session_exists(&info.id).await);

        manager.delete_session(&info.id).await.unwrap();
        assert!(!manager.session_exists(&info.id).await);
    }
}
