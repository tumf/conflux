---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/task_parser.rs:142
  - src/task_parser.rs:216
  - src/task_parser.rs:258
  - src/task_parser.rs:331
  - src/task_parser.rs:427
  - openspec/specs/code-maintenance/spec.md
---

# タスク進捗解決ロジックを共通化する

**Change Type**: implementation

## Problem / Context

`src/task_parser.rs` は 1359 行あり、タスク進捗の読み取り、archive 探索、worktree/base fallback、acceptance follow-up 追記が同じファイルに集約されています。特に `parse_change_with_worktree_fallback`、`parse_archived_change`、`parse_archived_change_with_worktree_fallback` は非推奨化されている一方で、`parse_progress_with_fallback` と同じ探索概念を個別に持ち続けています。

証拠:

- `src/task_parser.rs:142` に非推奨の `parse_change_with_worktree_fallback` が残っている。
- `src/task_parser.rs:216` に非推奨の `parse_archived_change` が残っている。
- `src/task_parser.rs:258` に非推奨の `parse_archived_change_with_worktree_fallback` が残っている。
- `src/task_parser.rs:331` の `parse_progress_with_fallback` が統一 fallback 順序を担っている。
- `src/task_parser.rs:427` 以降は acceptance follow-up 書き込み責務も同居している。

## Proposed Solution

タスク進捗の「場所解決」と「内容解析」と「follow-up 書き込み」を小さな内部ヘルパーへ分け、公開 API の挙動は維持します。既存の非推奨 API は互換性のため残してよいが、内部実装は共通の path resolution に委譲し、fallback 順序とエラー文言の後退を防ぎます。

## Acceptance Criteria

- 既存の公開関数シグネチャとエラー種別は維持される。
- worktree active → worktree archive → base archive → base active の探索順序は変わらない。
- archive ディレクトリの exact match 優先と date-prefixed fallback は変わらない。
- acceptance follow-up の追記・置換挙動は変わらない。
- `cargo fmt`、関連ユニットテスト、既定テストスイートが成功する。

## Explicit Completion Conditions

- `src/task_parser.rs` に探索順序を表す単一の内部ヘルパーまたは小さな helper 群が存在し、非推奨関数と `parse_progress_with_fallback` が重複実装ではなくそれを利用している。
- 既存テストに加え、fallback 順序と acceptance follow-up の characterization test が実装されている。
- `cargo test task_parser` と `cargo test` が成功する。

## Out of Scope

- タスク markdown の構文変更。
- `tasks.md` 以外の進捗ソース追加。
- OpenSpec archive 形式の変更。
