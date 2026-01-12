# Design: イベントシステムの統一

## Context

OpenSpec Orchestrator は、実行状態の通知にイベントベースのアーキテクチャを使用している。現在、2つの独立したイベント型が存在する：

1. `ParallelEvent`: parallel 実行モジュール用（25+ バリアント）
2. `OrchestratorEvent`: TUI 用（10+ バリアント）

これらは `parallel_event_bridge.rs` で変換されているが、このアーキテクチャは複雑さとメンテナンスコストを増加させている。

## Goals / Non-Goals

### Goals
- 単一の統一イベント型 `ExecutionEvent` を作成
- ブリッジレイヤーの削除による複雑さの低減
- 将来のイベント追加を容易にする

### Non-Goals
- 外部 API の変更（イベントは内部使用のみ）
- イベントの永続化やシリアライゼーション

## Decisions

### Decision 1: 統一イベント型の構造

```rust
#[derive(Debug, Clone)]
pub enum ExecutionEvent {
    // Lifecycle events
    ProcessingStarted(String),  // change_id
    ProcessingCompleted(String),
    ProcessingError { id: String, error: String },
    
    // Apply events
    ApplyStarted { change_id: String },
    ApplyCompleted { change_id: String },
    ApplyFailed { change_id: String, error: String },
    ApplyOutput { change_id: String, output: String },
    
    // Archive events
    ArchiveStarted(String),
    ArchiveCompleted(String),
    ArchiveFailed { change_id: String, error: String },
    ArchiveOutput { change_id: String, output: String },
    
    // Progress events
    ProgressUpdated { change_id: String, completed: u32, total: u32 },
    
    // Workspace events (parallel mode)
    WorkspaceCreated { change_id: String, path: String },
    WorkspaceResumed { change_id: String, path: String },
    
    // Merge events (parallel mode)
    MergeStarted { change_ids: Vec<String> },
    MergeCompleted { change_ids: Vec<String>, revision: String },
    MergeConflict { change_ids: Vec<String>, error: String },
    
    // Hook events
    HookStarted { hook_type: String, change_id: Option<String> },
    HookCompleted { hook_type: String, change_id: Option<String> },
    HookFailed { hook_type: String, change_id: Option<String>, error: String },
    
    // General events
    Log(LogEntry),
    Stopped,
    AllCompleted,
}
```

**理由**: 
- すべてのユースケースをカバーする包括的な設計
- 各バリアントに必要な情報を含める
- `change_id` を一貫して使用

### Decision 2: 移行戦略

1. 新しい `src/events.rs` を作成
2. `OrchestratorEvent` と `ParallelEvent` をエイリアスとして維持（移行期間中）
3. 段階的に各モジュールを新しい型に移行
4. 最終的にエイリアスと古い型を削除

**理由**: 段階的な移行により、大規模な破壊的変更を避ける

### Alternatives considered

1. **trait ベースの抽象化**: `Event` trait を作成し、各イベント型が実装
   - 却下理由: 過度な抽象化、enum の方がシンプル

2. **既存型の拡張**: `OrchestratorEvent` に parallel 用バリアントを追加
   - 却下理由: `tui` モジュール内に parallel 関連のコードを配置するのは不適切

## Risks / Trade-offs

- **Risk**: 大規模な変更による回帰バグ
  - Mitigation: 段階的な移行、各ステップでテストを実行

- **Trade-off**: 一時的なコード重複（移行期間中）
  - 許容理由: 移行完了後に削除可能

## Open Questions

- [ ] `LogEntry` 構造体は `events.rs` に移動すべきか、それとも別モジュールに維持すべきか
