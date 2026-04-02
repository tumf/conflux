---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/dispatch.rs
  - src/orchestration/rejection.rs
  - src/execution/state.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# Change: apply blocker を acceptance/rejection フローへ橋渡しする

**Change Type**: implementation

## Problem / Context

Conflux には acceptance が `Blocked` を返したときに rejection フロー（`REJECTED.md` 作成 → base コミット → resolve → worktree 削除）へ進む仕組みがある。しかし apply フェーズには blocker を構造化してランタイムへ渡す出口がなく、apply エージェントは blocker を `tasks.md` やログへ記録しても、実行サービスはそれを正式な状態遷移として扱えない。

その結果、apply が blocker を発見して `human_action_required` と `Implementation Blocker` を記録しても、未完了タスクが残っている限り apply は完了できず、acceptance にも進めない。実際の実行では空 WIP コミットの反復と stall detector による失敗へ陥る。

## Proposed Solution

apply フェーズに `blocked / reject-proposed` 相当の正式な出口を追加し、apply エージェントが blocker を検出した場合は `openspec/changes/<change_id>/REJECTED.md` を rejection 提案ファイルとして作成できるようにする。

ランタイムは以下を行う。

- apply 実行後、`REJECTED.md` の新規生成を検出した場合は tasks 未完了でも `apply blocked` として扱う
- `apply blocked` 状態の workspace は acceptance に進める
- acceptance は通常の pass/fail ではなく reject 承認を含む判定を行う
- acceptance が blocker/reject を承認した場合のみ、既存の rejection flow を完了させる
- acceptance が reject を承認しない場合は apply 継続またはエラーへ戻す

既存の `AcceptanceResult::Blocked` と `REJECTED.md` ベースの終端フローは維持しつつ、apply blocker がそこへ到達するための橋渡しを追加する。

## Acceptance Criteria

- apply エージェントが blocker を検出した場合、`tasks.md` の未完了項目が残っていても `REJECTED.md` により apply blocked 状態を表現できる
- orchestrator は apply blocked を単なる失敗や Created へ潰さず、acceptance に引き渡せる
- acceptance は apply 由来の reject proposal を検証し、承認時のみ rejection flow を実行する
- reject 承認後は既存どおり `REJECTED.md` が base に反映され、`openspec resolve <change-id>` と worktree cleanup が行われる
- apply blocker により empty WIP stall loop に入るケースが防止される

## Out of Scope

- acceptance verdict 全体の再設計
- `tasks.md` フォーマット全体を多値状態へ変更すること
- apply エージェントの一般的な実装戦略の再設計
