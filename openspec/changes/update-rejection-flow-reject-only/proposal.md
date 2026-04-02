---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/rejection.rs
  - src/parallel/dispatch.rs
  - src/serial_run_service.rs
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Change: reject flow は REJECTED.md 以外を base に持ち込まない

**Change Type**: implementation

## Problem / Context

現行の rejection flow は acceptance-confirmed blocked change に対して `openspec/changes/<change_id>/REJECTED.md` を base branch にコミットするが、その後に `openspec resolve <change_id>` を実行している。`openspec resolve` は change ディレクトリや関連ファイルに追加変更を加える可能性があり、reject flow の目的である「実装 worktree の内容を base に取り込まず、拒否理由だけを記録する」という境界を破る。

ユーザー要件は明確で、reject flow では `REJECTED.md` 以外を merge / stage / commit してはならない。apply worktree 側の code/spec/tasks 差分は reject 理由の証跡としてのみ扱い、base へ持ち込まない。

## Proposed Solution

rejection flow を `REJECTED.md` 専用の base-side marker commit として定義し直す。

- rejection flow は base branch に `openspec/changes/<change_id>/REJECTED.md` だけを書き込む
- base branch では `REJECTED.md` 以外の change files を stage / commit しない
- reject flow から `openspec resolve <change_id>` を外す
- rejected change の終端性、一覧からの除外、再キュー禁止は runtime state と `REJECTED.md` marker によって判定する
- worktree cleanup は reject marker commit の成否にのみ従い、`resolve` の成否に依存させない

## Acceptance Criteria

- rejection flow の base commit に含まれる変更は `openspec/changes/<change_id>/REJECTED.md` のみである
- reject flow 実行時に `openspec resolve <change_id>` は呼び出されない
- apply/generated worktree 側の code/spec/tasks 差分は base に merge されない
- rejected change は `REJECTED.md` marker と runtime state により `rejected` として扱われる
- reject marker commit 後に worktree cleanup が行われ、reject flow が `resolve` 不在でも完了できる

## Out of Scope

- OpenSpec CLI 自体の `resolve` 動作変更
- apply blocker proposal の内容生成ルール変更
- rejected change の long-term archival policy 全体の再設計
