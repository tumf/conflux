---
change_type: implementation
priority: high
dependencies: []
references:
  - src/execution/apply.rs
  - src/execution/state.rs
  - src/parallel/dispatch.rs
  - openspec/specs/parallel-execution/spec.md
  - ~/.local/state/cflx/logs/avacuscc-dbot-f6307a82/2026-05-01.log
---

# Change: apply-generated REJECTED.md を即 Rejecting handoff にする

**Change Type**: implementation

## Problem/Context

`add-dbot-skill-toggles` の apply は `openspec/changes/add-dbot-skill-toggles/REJECTED.md` を生成し、`human_action_required: acceptance must confirm rejection proposal` を出力していた。しかし runtime はその apply run を `Rejecting` handoff として終了せず、`tasks.md` が 7/9 の未完了であることだけを見て apply retry を継続した。

その後、同じ `REJECTED.md` / blocker state を繰り返し WIP snapshot し、5 回連続の empty WIP commit と判定され、`Stall detected for add-dbot-skill-toggles after 5 empty WIP commits (apply)` で terminal Error になった。

現行 `src/execution/apply.rs` の early completion detection は `APPLY_BLOCKED/marker.md` と tasks complete のみを見ており、worktree-local `REJECTED.md` を apply completion kind として扱わない。既存 spec は `REJECTED.md` が生成された場合、通常 acceptance/apply retry ではなく dedicated rejecting stage に入ることを求めている。

## Proposed Solution

Apply loop の completion detection に worktree-local `openspec/changes/<change_id>/REJECTED.md` を追加し、検出時は `RejectingHandoff` として apply loop を終了する。

`RejectingHandoff` は empty WIP stall 判定を通さず、apply result から caller に伝播され、parallel dispatch は `Rejecting` status/event を出して `run_rejection_review` 経路へ進む。`APPLY_BLOCKED` は従来通り resumable stalled handoff、`REJECTED.md` は dedicated rejecting handoff として分離する。

## Acceptance Criteria

- Apply command が worktree-local `REJECTED.md` を生成した場合、tasks.md に未完了タスクが残っていても apply retry を継続しない。
- `REJECTED.md` handoff 後は empty WIP stall detector による terminal Error に落ちない。
- Parallel mode は `REJECTED.md` handoff を `Rejecting` として扱い、rejection review を実行する。
- `APPLY_BLOCKED/marker.md` handoff と `REJECTED.md` handoff は別の outcome として維持される。

## Explicit Completion Conditions

- `src/execution/apply.rs` が `REJECTED.md` を apply completion kind として検出し、stall check 前に apply loop を抜ける。
- `src/parallel/dispatch.rs` または関連 orchestration 経路が apply result の rejected handoff を `Rejecting` review に接続する。
- Regression test が、未完了 tasks + generated `REJECTED.md` + empty WIP snapshot の組み合わせでも stall error にならず rejecting handoff になることを検証する。
- `cflx openspec validate fix-rejected-apply-handoff --strict --evidence warn` が成功する。

## Out of Scope

- Rejection review の verdict semantics の変更。
- `APPLY_BLOCKED` marker format の変更。
- apply agent prompt の全面再設計。
