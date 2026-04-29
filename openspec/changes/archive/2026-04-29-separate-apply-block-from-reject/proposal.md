---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/agent-prompts/spec.md
  - src/execution/apply.rs
  - src/orchestration/rejection.rs
  - src/parallel/dispatch.rs
  - src/execution/state.rs
  - src/events.rs
  - src/vcs/mod.rs
---

# Change: apply failure から BLOCK と REJECT を分離する

**Change Type**: implementation

## Premise / Context

- 現行 parallel execution は、apply が `openspec/changes/<change_id>/REJECTED.md` を生成すると dedicated `rejecting` に遷移し、rejecting review が `REJECTION_REVIEW: RESUME` を返すと無条件で `Applying` に戻す（`openspec/specs/parallel-execution/spec.md`）。
- この設計だと「この apply attempt は進められないが change 全体はまだ有効」という recoverable blocker と、「change 自体を閉じるべき rejection proposal」が同じ経路に乗る。
- 直近の `refactor-extract-git-sync-test-fixtures` では、apply-generated rejection proposal を rejecting review が却下しても `Applying` に戻され、同じ blocker が再生成されるループが発生した。
- `agent-prompts` には apply が Implementation Blocker を記録する契約がすでに存在するため、apply outcome を `BLOCK` と `REJECT` に分離する拡張余地がある。

## Problem / Context

Conflux は現在、apply failure を「再開可能な blocked state」と「change 終端に向かう rejection proposal」に十分分離できていない。結果として、recoverable blocker まで `REJECTED.md` ベースの rejection フローに入り、rejecting review が reject を受け入れなかった場合でも runtime がそのまま `Applying` に戻してしまう。

この挙動では、追加情報・仕様判断・fixture 再設計・依存解消が必要な change を「今すぐ再試行可能な apply work」と誤分類し、同一 blocker の再発・レビュー負荷・worktree 遷移のノイズを生む。change を残したまま停止し、条件がそろえば同じ worktree から再開できる `Blocked` の意味を runtime / prompt / spec で明確化する必要がある。

## Proposed Solution

apply 側から outcome を明示的に分離し、recoverable blocker を rejection proposal と別経路で扱う。

- apply は **default を BLOCK** とし、再開条件を書ける失敗は `BLOCKED` として報告する。
- apply は `REJECTED.md` を「change 自体の reject 提案」を出すときにのみ生成する。
- runtime は apply-generated `BLOCKED` を新しい non-terminal `Blocked` activity/state に遷移させ、worktree・WIP・task progress を維持する。
- rejecting review は rejection proposal を `CONFIRM` / `RESUME` / `BLOCK` のいずれかで返し、reject を却下しつつ即 apply 再開すべきでない場合は `Blocked` へ返す。
- `Blocked` は追加情報・仕様修正・依存解消・明示的 retry により `Applying` へ復帰できる resume-capable stop state とする。

## Acceptance Criteria

- apply は recoverable blocker と terminal rejection proposal を別 outcome として報告できる。
- recoverable blocker では worktree-local `REJECTED.md` を生成せず、change は `Blocked` として保持される。
- rejecting review が reject proposal を却下し、かつ即 apply 再開が不適切と判断した場合、change は `Blocked` に遷移する。
- `Blocked` change は worktree / WIP / task progress / blocker reason を保持し、追加情報または明示的 retry により同じ change を再開できる。
- `Rejected` は引き続き terminal state であり、base branch に durable `REJECTED.md` を残す終端処理としてのみ使われる。
- TUI / Web / reducer / scheduler が `Blocked` を `Rejected` や `Applying` と混同せず表示・遷移できる。

## Explicit Completion Conditions

- OpenSpec delta が apply outcome separation、rejecting verdict semantics、blocked state retention を canonical spec として記述している。
- proposal tasks に runtime state / event / UI / prompt / validation まで含まれ、実装 agent が隠れた前提なしに着手できる。
- `Blocked` の保持対象（worktree, WIP, task progress, blocker metadata）と復帰条件が spec 上で明示されている。
- rejection proposal 不採用時の遷移先が `Applying` 一択でなくなることを示す scenario が追加されている。
- `cflx openspec validate separate-apply-block-from-reject --strict --evidence warn` が成功する。

## Out of Scope

- 既存すべての blocked/rejected archived history の自動移行戦略
- dependency blocked と apply blocked の UI 文言統一の最終 polish
- proposal authoring UX 全般の再設計
