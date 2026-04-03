---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - src/execution/apply.rs
  - src/orchestration/rejection.rs
  - skills/cflx-workflow/SKILL.md
  - skills/cflx-workflow/references/cflx-accept.md
---

# Change: Split rejection review from acceptance

**Change Type**: implementation

## Problem / Context

現在の blocked handoff は、`apply` が `openspec/changes/<change_id>/REJECTED.md` を生成した後も rejection review を通常の acceptance フローに内包している。これにより、実装品質の検証である acceptance と、change を reject すべきかの審査が同じステップに混在している。

既存仕様では apply-generated `REJECTED.md` は rejection proposal であり、acceptance がそれを confirm したときのみ terminal rejection になる。また、base branch に反映されるのは `REJECTED.md` のみで、他の worktree 変更は捨てられる。

ユーザー要件として、workflow 上 `accept` と reject review は分離されなければならない。新しい専用ステータス `rejecting` を導入し、`apply` が rejection proposal を生成した change は通常 acceptance に進まず `rejecting` に入る。`rejecting` は `REJECTED.md` を審査し、(1) reject を認めて `REJECTED.md` のみを base に反映するか、(2) reject を棄却して `REJECTED.md` を除去し apply に戻すか、のどちらかを決定する。さらに reject 棄却時には、`tasks.md` に reject ではない解決方針のタスクを追加しなければならない。

## Proposed Solution

- apply-generated `REJECTED.md` を acceptance handoff ではなく rejection-review handoff として扱う。
- shared runtime state / display status に `rejecting` を追加し、通常 acceptance から独立した active stage とする。
- `rejecting` は acceptance とは別の専用 review operation として `REJECTED.md` をレビューし、最終行に machine-readable verdict marker を 1 つだけ出力する。
- rejecting review の verdict marker は `REJECTION_REVIEW: CONFIRM` または `REJECTION_REVIEW: RESUME` のみとし、runtime はこの dedicated protocol を parse して次段に進む。
- `confirm_rejection` / `REJECTION_REVIEW: CONFIRM` の場合は既存の rejection flow を維持し、base branch には `openspec/changes/<change_id>/REJECTED.md` のみを commit する。
- `resume_apply` / `REJECTION_REVIEW: RESUME` の場合は worktree から `REJECTED.md` を除去し、`tasks.md` に reject ではない解決タスクを追加した上で `applying` に戻す。
- apply-generated `REJECTED.md` handoff は通常 acceptance に送らず rejecting review にルーティングし、通常 acceptance は `REJECTED.md` のない changes のみを扱う。
- parallel / serial の resume・表示・API・イベントは `rejecting` を独立した実行段階として扱う。
- rejecting 導入に伴うエージェント挙動の仕様変更は product code だけでなく `skills/cflx-workflow/` 配下の workflow skill source を canonical source として更新する。

## Acceptance Criteria

- apply が `REJECTED.md` を生成した change は通常 acceptance ではなく `rejecting` に遷移する。
- `rejecting` は rejection proposal の妥当性を判定し、runtime が parse 可能な dedicated verdict marker として `REJECTION_REVIEW: CONFIRM` または `REJECTION_REVIEW: RESUME` の二択だけを返す。
- `confirm_rejection` / `REJECTION_REVIEW: CONFIRM` 時、base branch に反映されるのは `REJECTED.md` のみである。
- `resume_apply` / `REJECTION_REVIEW: RESUME` 時、worktree の `REJECTED.md` は削除され、`tasks.md` に reject 以外の解決タスクが追加され、change は `applying` に戻る。
- apply-generated `REJECTED.md` handoff は通常 acceptance ではなく rejecting review に入り、`ACCEPTANCE: BLOCKED` はこの handoff の最終判断に使われない。
- TUI / Web API / shared state の表示で `rejecting` が独立した active status として観測できる。
- resume 時に `REJECTED.md` が存在する非 terminal workspace は `accepting` ではなく `rejecting` に復元される。
- workflow skill source (`skills/cflx-workflow/SKILL.md` と関連 reference) が rejecting 分離後の handoff / verdict / task-update semantics と整合する。

## Out of Scope

- `REJECTED.md` 以外の proposal / tasks / spec / code 変更を base branch に反映する rejection merge モード
- rejection review の結果に第三の判定（保留、手動承認待ち等）を追加すること
- rejection review の根拠を外部 DB や別メタデータファイルに永続化すること
