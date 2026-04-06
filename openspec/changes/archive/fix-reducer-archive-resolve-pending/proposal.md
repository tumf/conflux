---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/orchestration-state/spec.md
  - src/orchestration/state.rs
  - src/tui/orchestrator.rs
---

# Fix: Reducer archive transition respects active project resolve

**Change Type**: implementation

## Premise / Context

- ユーザは「他の change が resolve 中に archive が終わったとき、`merge wait` ではなく `resolve pending` になるべき」と指摘している
- 直前の確認で、この判定は **project スコープ** の話であることが明示された
- canonical spec `openspec/specs/orchestration-state/spec.md` にはすでに `archive-completes-while-resolve-active` があり、`ChangeArchived` 後に `ResolveWait` へ遷移すべきことが定義済みである
- しかし reducer 実装 `src/orchestration/state.rs` は `ExecutionEvent::ChangeArchived` で常に `WaitState::MergeWait` を設定している
- 既存の archived proposal `fix-tui-archived-resolve-wait` は TUI orchestrator 層の post-archive dispatch を修正しているが、reducer 自体の遷移条件は未修正である

## Problem / Context

`OrchestratorState` は project スコープの共有 reducer であり、表示状態の単一ソースである。それにもかかわらず、parallel mode の `ChangeArchived` 処理は他の change の `Resolving` 状態を見ずに一律で `MergeWait` を設定している。

このため、project 内で別 change の resolve が進行中に archive が完了した change は、spec が要求する `resolve pending` ではなく `merge wait` を表示しうる。TUI orchestrator 側に post-archive dispatch が存在しても、reducer 単体の遷移ロジックとその単体テストが spec を満たしていないため、共有状態の正規ソースとしての整合性が崩れる。

## Proposed Solution

`src/orchestration/state.rs` の `ExecutionEvent::ChangeArchived` における parallel-mode 分岐を修正し、**同一 project 内の他 change が `ActivityState::Resolving` なら `WaitState::ResolveWait`、そうでなければ `WaitState::MergeWait`** に遷移させる。

この変更にあわせて reducer 単体テストを追加し、以下を明示的に検証する。

1. 他 change が resolving 中のとき、archived change は `resolve pending` になる
2. resolving 中の change がいないとき、従来どおり `merge wait` になる
3. 判定は project スコープで行われ、change 自身の archive 完了だけでは `resolve pending` にしない

## Acceptance Criteria

1. `OrchestratorState` が parallel mode で `ChangeArchived` を処理するとき、同一 project 内の別 change が `Resolving` 中なら archived change の wait state は `ResolveWait` になる
2. 上記ケースで derived display status は `resolve pending` を返す
3. 同一 project 内に resolving 中の別 change が存在しない場合、`ChangeArchived` は引き続き `MergeWait` になり、derived display status は `merge wait` を返す
4. reducer 単体テストに、archive-during-active-resolve の回帰テストと no-active-resolve の維持テストが追加される
5. proposal strict validation が成功する

## Out of Scope

- TUI orchestrator の post-archive dispatch ロジックの再設計
- headless parallel executor の merge scheduling 変更
- canonical spec の新規追加（既存 spec を実装に一致させる変更のため）
