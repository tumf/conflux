# Change: Resolve待ちステータスの追加

## Why
並列実行でresolveがシリアライズされている間、archive済みchangeが`not queued`に戻ってしまい、待機中であることが見えません。結果としてSpaceキーでキュー状態を変更できてしまい、誤操作や状態の不整合が発生します。

## What Changes
- `QueueStatus::ResolveWait`を追加し、archive済みでresolve待ちのchangeを明示する
- `update-workspace-archive-detection`で実装された`WorkspaceState::Archived`判定を活用し、worktree内でarchive済みだがmerge未完了のchangeを`ResolveWait`として識別する
- TUIの自動更新で`ResolveWait`を保持し、`NotQueued`へのリセットを防止する
- `ResolveWait`はキュー操作の対象外とし、Space/@操作で状態を変更できないようにする

## Impact
- Affected specs: `openspec/specs/tui-architecture/spec.md`
- Affected code: `src/tui/types.rs`, `src/tui/state/events/helpers.rs`, `src/tui/state/mod.rs`, `src/tui/render.rs`, `src/parallel/mod.rs` (WorkspaceState判定呼び出し)
