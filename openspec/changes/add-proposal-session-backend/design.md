# Design: Proposal Session Backend

## Architecture Overview

```
Dashboard WebSocket        Rust Server                    ACP Agent
     Client          ┌─────────────────────┐         (opencode acp)
                     │                     │
  prompt ──────────► │  ProposalSession     │ ──────► session/prompt
                     │  Manager             │
  elicitation_resp ► │                     │ ──────► elicitation resp
                     │  ┌─AcpClient─────┐  │
  cancel ──────────► │  │ stdin writer   │  │ ──────► session/cancel
                     │  │ stdout reader  │  │
  ◄── agent_chunk    │  │ process handle │  │ ◄────── session/update
  ◄── tool_call      │  └───────────────┘  │
  ◄── elicitation    │                     │
  ◄── turn_complete  └─────────────────────┘
```

## Key Components

### ProposalSessionConfig (`src/config/types.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalSessionConfig {
    pub acp_command: String,                    // default: "opencode"
    pub acp_args: Vec<String>,                  // default: ["acp"]
    pub acp_env: HashMap<String, String>,       // default: {}
    pub session_inactivity_timeout_secs: u64,   // default: 1800
}
```

### AcpClient (`src/server/acp_client.rs`)

Wraps a single ACP subprocess:
- Spawns process with `tokio::process::Command` in the worktree directory
- Reads stdout line-by-line for JSON-RPC messages
- Writes to stdin for outgoing JSON-RPC messages
- Handles the `initialize` → `initialized` handshake
- Client capabilities: `{ fs: { readTextFile: true, writeTextFile: true }, terminal: true, elicitation: { form: {} } }`
- Maps ACP `session/update` notifications to typed Rust enums

### ProposalSession State

```rust
pub struct ProposalSession {
    pub id: String,
    pub project_id: String,
    pub worktree_path: PathBuf,
    pub worktree_branch: String,
    pub acp_client: AcpClient,
    pub acp_session_id: String,
    pub status: ProposalSessionStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

pub enum ProposalSessionStatus {
    Active,
    Merging,
    Closed,
}
```

### ProposalSessionManager

- `HashMap<String, ProposalSession>` behind `RwLock`
- Generates session IDs: `ps_<nanoid>`
- Creates worktree branch: `proposal/<session_id>`
- Background tokio task for inactivity timeout scanning (every 60s)

### WebSocket Message Protocol

Client → Server:
```json
{ "type": "prompt", "text": "..." }
{ "type": "elicitation_response", "request_id": "...", "action": "accept|decline|cancel", "content": { ... } }
{ "type": "cancel" }
```

Server → Client:
```json
{ "type": "agent_message_chunk", "text": "..." }
{ "type": "tool_call", "tool_call_id": "...", "title": "...", "kind": "...", "status": "pending" }
{ "type": "tool_call_update", "tool_call_id": "...", "status": "in_progress|completed|failed", "content": [...] }
{ "type": "elicitation", "request_id": "...", "mode": "form", "message": "...", "schema": { ... } }
{ "type": "turn_complete", "stop_reason": "end_turn|max_tokens|cancelled" }
{ "type": "changes_detected", "changes": [{ "id": "...", "title": "..." }] }
{ "type": "error", "message": "..." }
```

### Dirty Worktree Close Flow

1. Client: `DELETE /proposal-sessions/{id}` with `{ "force": false }`
2. Server: `git status --porcelain` in worktree
3. If dirty → 409 response: `{ "status": "dirty", "message": "...", "uncommitted_files": [...] }`
4. Client: re-sends with `{ "force": true }`
5. Server: kill ACP process → remove worktree → remove session → 200

### Merge Flow

1. Client: `POST /proposal-sessions/{id}/merge`
2. Server: check `git status --porcelain` → if dirty, 409 error
3. `git checkout {entry.branch}` in bare repo context
4. `git merge {worktree_branch}` → if conflict, 409 error
5. Remove worktree, kill ACP, remove session → 200

## Dependencies

- `tokio` (subprocess, channels, timers)
- `serde_json` (JSON-RPC serialization)
- `nanoid` or `uuid` (session ID generation — check existing usage first)
- `chrono` (timestamps — already used)
