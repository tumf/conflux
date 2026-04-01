## Context

The server mode dashboard loses all state on browser reload or server restart. Proposal sessions (backed by ACP subprocesses and git worktrees) are purely in-memory. The existing `ServerDb` (SQLite via rusqlite) already persists change events, logs, and change states — extending it for sessions and UI preferences is a natural fit.

## Goals / Non-Goals

**Goals**:
- Browser reload restores project selection, active session tab, and chat history
- Server restart restores sessions whose worktrees still exist on disk
- Minimal schema additions — reuse existing `ServerDb` patterns

**Non-Goals**:
- ACP conversation context restoration (the ACP subprocess is stateless from our perspective; we only persist displayed chat messages)
- Real-time sync between multiple browser tabs (single-user model)

## Decisions

### SQLite schema extension (migration v2)

Three new tables added in a single migration block gated by `user_version < 2`:

```sql
CREATE TABLE ui_state (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE proposal_sessions (
    id               TEXT PRIMARY KEY,
    project_id       TEXT NOT NULL,
    worktree_path    TEXT NOT NULL,
    worktree_branch  TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'active',
    acp_session_id   TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    last_activity    TEXT NOT NULL
);

CREATE TABLE proposal_session_messages (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id   TEXT NOT NULL REFERENCES proposal_sessions(id) ON DELETE CASCADE,
    message_id   TEXT NOT NULL,
    role         TEXT NOT NULL,
    content      TEXT NOT NULL,
    timestamp    TEXT NOT NULL,
    turn_id      TEXT,
    is_thought   INTEGER,
    tool_calls   TEXT,
    seq          INTEGER NOT NULL
);
```

**Rationale**: `ui_state` is a generic key-value store (not session-specific) because future UI preferences (theme, panel widths, etc.) can reuse it. `proposal_session_messages.tool_calls` stores JSON-serialized `Vec<ProposalSessionToolCallRecord>`.

### DI of ServerDb into ProposalSessionManager

`ProposalSessionManager` receives `Option<Arc<ServerDb>>` at construction. When `None` (e.g., tests), persistence is skipped. This avoids making tests depend on a real database.

### Session restoration flow (server startup)

```
run_server()
  ├─ ServerDb::new()
  ├─ db.load_active_proposal_sessions()   // status IN ('active', 'timed_out')
  ├─ For each session row:
  │   ├─ Check worktree_path exists on disk
  │   │   └─ If missing → db.delete_proposal_session(id) + skip
  │   ├─ AcpClient::spawn() → initialize() → create_session()
  │   ├─ db.load_proposal_session_messages(id) → session.message_history
  │   ├─ Update acp_session_id in DB (new ACP session after re-spawn)
  │   └─ Insert into ProposalSessionManager.sessions
  └─ Continue with normal server startup
```

### Message persistence strategy

Messages are written to DB at turn boundaries (not per-chunk) to balance durability and write frequency:
- `record_user_prompt()` → immediate insert
- `complete_active_turn()` → insert the full assistant message accumulated during the turn
- Crash mid-turn: the partial assistant message is lost (acceptable — ACP will not resume the turn anyway)

### Activity write throttling

`ProposalSession` gets a `last_db_activity_write: DateTime<Utc>` field. The `touch()` method only calls `db.update_proposal_session_activity()` if ≥60 seconds have elapsed since the last DB write, reducing write amplification during active streaming.

### Frontend restoration flow

```
App initializes
  ├─ WebSocket connects → receives FullState (includes ui_state map)
  ├─ If ui_state.selected_project_id exists in project list → SELECT_PROJECT
  ├─ Fetch proposal sessions for selected project
  ├─ If ui_state.active_proposal_session_id exists and is active → SET_ACTIVE_PROPOSAL_SESSION
  └─ If stored IDs are stale → DELETE from ui_state
```

Selection changes (`SELECT_PROJECT`, `SET_ACTIVE_PROPOSAL_SESSION`) fire-and-forget a `PUT /api/v1/ui-state/{key}` call. Null selections call `DELETE`.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| ACP re-spawn fails on startup (binary missing, port conflict) | Log warning and mark session as `timed_out` in DB; user can manually close |
| Large message history in SQLite | Messages are text-only; even 1000 messages per session is <1MB |
| Migration v2 on existing DBs | Additive-only schema; no table alterations; gated by `user_version < 2` |

## Open Questions

None — design is straightforward extension of existing patterns.
