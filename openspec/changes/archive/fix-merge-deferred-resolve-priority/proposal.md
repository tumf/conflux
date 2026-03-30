---
change_type: implementation
priority: high
dependencies:
  - fix-resolve-merge-race-condition
references:
  - src/parallel/merge.rs
  - src/parallel/mod.rs
  - src/tui/command_handlers.rs
---

# Fix: archive完了後のmerge試行で resolve 進行中チェックを base dirty より優先する

**Change Type**: implementation

## Problem/Context

Change A が resolving 中、Change B が archiving → archive 完了後に merge を試行するとき、以下の問題が発生する：

1. `attempt_merge()` は最初に `base_dirty_reason()` をチェックする（`src/parallel/merge.rs` line 274）
2. A の resolve 操作により base が dirty な場合、`MergeDeferred` が返る
3. `is_dirty_reason_auto_resumable()` は MERGE_HEAD の存在のみで `auto_resumable` を判定する
4. resolve のタイミングにより MERGE_HEAD が存在しない場合（resolve コマンド実行前、またはコンフリクト解決中の中間状態）、`auto_resumable = false` と判定される
5. 結果として `MergeWait`（手動介入必要）に遷移してしまう

**期待動作**: 他の change が resolving 中であれば、base dirty の理由に関わらず `ResolveWait`（キュー待ち）に遷移すべき。

### 根本原因

resolving はプロジェクトレベルの属性であり、TUI の `is_resolving` フラグではなく、`ParallelExecutor` が既に持っている `auto_resolve_count`（自動 resolve）と `manual_resolve_count`（TUI M キー由来）で判定可能。merge 試行時にこれらのカウンターを先にチェックし、resolve が進行中なら `base_dirty_reason()` の結果に関わらず `auto_resumable = true` として扱うべき。

### 影響箇所

1. **parallel executor** の `handle_archive_completed()` → `attempt_merge()`: archive 完了後の自動 merge 試行
2. **TUI command handler** の `ResolveMerge` ハンドラ（`src/tui/command_handlers.rs` line 581）: M キーによる resolve 開始前の dirty チェック。こちらは `manual_resolve_count` にアクセスできるため同様に防御可能。

## Proposed Solution

### 1. `attempt_merge()` に resolve 進行中の早期リターンを追加

`base_dirty_reason()` をチェックする前に、`auto_resolve_count + manual_resolve_count > 0` であれば `MergeAttempt::Deferred("Resolve in progress")` を返す。これにより呼び出し元で `auto_resumable = true` として扱われる。

`attempt_merge()` は現在 `&self` で `ParallelExecutor` のフィールドにアクセスできるため、`auto_resolve_count` を直接参照可能。`manual_resolve_count` は `Option<Arc<AtomicUsize>>` なので `unwrap_or(0)` で参照。

### 2. TUI command handler の防御チェック

`src/tui/command_handlers.rs` の `ResolveMerge` ハンドラ（line 581）で `base_dirty_reason` チェック前に `manual_resolve_count` を確認する。ただし依存先 proposal（`fix-resolve-merge-race-condition`）で `is_resolving` フラグの即時設定が修正されれば、TUI state 側の `resolve_merge()` で既にキューに入るため、このハンドラに到達する可能性は低い。防御的に追加する。

## Acceptance Criteria

1. Change A が resolving 中に Change B の archive が完了した場合、B は `ResolveWait` に遷移し resolve キューに追加される（`MergeWait` にならない）
2. resolving 中でない場合の既存の dirty base 判定（MERGE_HEAD → auto_resumable, uncommitted changes → manual）は維持される
3. resolve 完了後、キューに入った B の resolve が自動的に開始される

## Out of Scope

- `is_dirty_reason_auto_resumable()` のロジック変更（根本原因は判定順序の問題であり、auto_resumable の分類自体は正しい）
- Web UI 側の同等修正
