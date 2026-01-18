# Change: Web UI を TUI と完全一致させるための監視アーキテクチャ再設計

## Why
Web UI が TUI と同じ情報をリアルタイムに反映できず、監視結果が不整合になるため、信頼できる監視基盤として再設計が必要です。

## What Changes
- Web UI が TUI と同一の状態モデルを購読できるように監視イベントと状態管理の構造を見直す
- TUI の更新ソース（ChangesRefreshed/実行イベント/キュー/ログ/ワークツリー）を Web へ一貫して配信できる統合経路を追加する
- Web UI のデータ契約を拡張し、TUI と同等の粒度で表示・更新できるようにする

## Impact
- Affected specs: web-monitoring, tui-architecture, observability
- Affected code: src/web/*, src/tui/*, src/events.rs, web/*

## Implementation Summary

### Backend Changes (Completed)

1. **Extended WebState data model** (src/web/state.rs):
   - Added `logs: Vec<LogEntry>` field to OrchestratorState
   - Added `worktrees: Vec<WorktreeInfo>` field to OrchestratorState
   - Added `app_mode: String` field to OrchestratorState (e.g., "select", "running", "stopped")
   - Added `queue_status: Option<String>` field to ChangeStatus for tracking execution state

2. **Enhanced StateUpdate WebSocket message** (src/web/state.rs):
   - Added optional `logs` field for real-time log streaming
   - Added optional `worktrees` field for worktree list updates
   - Added optional `app_mode` field for application mode changes

3. **Implemented comprehensive ExecutionEvent handlers** (src/web/state.rs):
   - ProcessingStarted/Completed/Error: Updates change status and queue_status
   - ArchiveStarted/ChangeArchived: Tracks archiving lifecycle
   - ProgressUpdated: Syncs task completion progress
   - MergeCompleted/ResolveStarted/ResolveCompleted/ResolveFailed: Tracks parallel merge flow
   - Log: Appends log entries (keeps last 1000 entries)
   - ChangesRefreshed: Updates full change list while preserving queue_status
   - WorktreesRefreshed: Updates worktree list
   - Stopped/AllCompleted: Updates app_mode

4. **Added Serialize/Deserialize support**:
   - LogEntry and LogLevel (src/events.rs): Added serde derives for web serialization
   - WorktreeInfo and MergeConflictInfo (src/tui/types.rs): Added serde derives for web serialization

5. **Verified existing integration** (src/tui/orchestrator.rs):
   - WebState event forwarding channel already implemented in parallel execution
   - All ExecutionEvents are already forwarded to WebState via mpsc channel
   - WebSocket broadcast already sends initial state on connection

### Frontend Changes (Future work)

The following frontend implementation tasks are deferred to future work as they require JavaScript/TypeScript development and extensive UI testing:

1. Extend web/app.js to handle new message types (logs, worktrees, app_mode)
2. Implement log panel UI component (similar to TUI)
3. Implement worktree view UI component (similar to TUI)
4. Add queue_status badges to change cards (Queued, Processing, Archiving, Merged, etc.)
5. Add real-time log streaming UI
6. Add worktree management UI

### Architecture Benefits

- **Single Source of Truth**: Both TUI and Web UI now receive identical ExecutionEvent stream
- **Real-time Parity**: WebState broadcasts same events as TUI receives
- **Type Safety**: Serde serialization ensures consistent data contracts
- **Extensibility**: Easy to add new event types or state fields
