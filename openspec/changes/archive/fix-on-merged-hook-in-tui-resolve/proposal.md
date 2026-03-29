---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/command_handlers.rs
  - src/parallel/merge.rs
  - openspec/specs/hooks/spec.md
---

# Change: TUI ResolveMerge 経路で on_merged フックが実行されない不具合を修正

**Change Type**: implementation

## Why

TUI の `ResolveMerge` コマンドハンドラ (`src/tui/command_handlers.rs`) は `resolve_deferred_merge()` を呼び出すが、この関数内で生成される `ParallelExecutor` は `hooks: None` のまま構築される。そのため `on_merged` フック（例: `make bump-patch`）が実行されないまま `ResolveCompleted` イベントが送信され、TUI 上で merged ステータスに遷移してしまう。

同ファイル内の `BranchMerge`（手動マージ）ハンドラ (L444-499) では `on_merged` フックが正しく実行されてから `BranchMergeCompleted` を送信しており、`ResolveMerge` ハンドラだけこのパターンが欠落している。

## What Changes

- `src/tui/command_handlers.rs` の `ResolveMerge` ハンドラ: `resolve_deferred_merge()` 成功後、`ResolveCompleted` イベント送信前に `on_merged` フックを実行する
- BranchMerge ハンドラ (L444-499) と同じパターンを適用: config から HookRunner を構築 → task counts 取得 → `on_merged` 実行 → イベント送信

## Impact

- Affected specs: hooks
- Affected code: `src/tui/command_handlers.rs`

## Acceptance Criteria

- TUI の ResolveMerge 成功時に `on_merged` フックが実行される
- `on_merged` フックの完了後に `ResolveCompleted` イベントが送信される（merged ステータス遷移は `on_merged` 完了後）
- BranchMerge ハンドラの既存動作に影響しない
- 既存テスト `test_on_merged_hook_execution` が引き続きパスする

## Out of Scope

- `resolve_deferred_merge()` 内の `ParallelExecutor` に hooks を渡す設計変更（caller 側で実行する方が BranchMerge と一貫性がある）
