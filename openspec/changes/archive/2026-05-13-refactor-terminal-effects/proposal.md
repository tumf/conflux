---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/orchestration/state.rs:1023
  - src/orchestration/state.rs:1193
  - src/orchestration/state.rs:1347
  - src/orchestration/state.rs:1442
  - src/orchestration/state.rs:1525
  - openspec/specs/code-maintenance/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# オーケストレーション状態遷移の副作用を整理する

**Change Type**: implementation

## Problem / Context

`src/orchestration/state.rs` は 4389 行あり、Reducer の状態、コマンド処理、ExecutionEvent 処理、テストが単一ファイルに密集しています。`apply_command` と `apply_execution_event` は状態遷移だけでなく、`resolve_wait_queue` / `reject_wait_queue` の副作用、blocked metadata のクリア、terminal/activity/wait の組み合わせを各 match arm で直接操作しており、将来の状態追加時に不変条件を崩しやすい構造です。

証拠:

- `src/orchestration/state.rs:1023` の `apply_command` がコマンドごとの状態変更と queue 副作用を直接持つ。
- `src/orchestration/state.rs:1193` の `apply_execution_event` が大きな event reducer として多数の状態遷移を処理している。
- `src/orchestration/state.rs:1347` 以降で workspace status 同期が reducer 内に同居している。
- `src/orchestration/state.rs:1442` 以降で archive event の terminal/wait/activity 副作用が直接記述されている。
- `src/orchestration/state.rs:1525` 以降で merge/resolve event の queue 副作用が同じ関数内に重なっている。

## Proposed Solution

Reducer の公開挙動を変えずに、状態遷移の副作用を小さな内部 helper に抽出します。特に「base-mutating lane wait queue の操作」「terminal 化時の共通クリーンアップ」「blocked metadata の設定/解除」「success event が terminal error を上書きできる条件」を明示的な内部関数にまとめ、既存 invariant test を強化します。

## Acceptance Criteria

- `apply_command` と `apply_execution_event` の公開結果、`ReduceOutcome`、表示 status は変わらない。
- `resolve_wait_queue` と `reject_wait_queue` の追加・削除タイミングは既存テストで固定される。
- `ChangeRuntimeState::invariants_hold` と `OrchestratorState::global_invariants_hold` が refactor 後も成功する。
- terminal state、active state、wait state の組み合わせに関する既存 regression test が通る。
- `cargo fmt`、関連ユニットテスト、既定テストスイートが成功する。

## Explicit Completion Conditions

- `src/orchestration/state.rs` に queue cleanup、terminal transition、blocked metadata transition の内部 helper が存在し、複数 match arm の重複操作がそこへ集約されている。
- コマンド処理・イベント処理の characterization test が追加または整理され、少なくとも archive→resolve、reject wait、dequeue、terminal error retry の代表経路を検証している。
- `cargo test orchestration::state` 相当の状態テストと `cargo test` が成功する。

## Out of Scope

- 新しい状態や表示 status の追加。
- serial/parallel の意味論変更。
- TUI/Web の表示デザイン変更。
