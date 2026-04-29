## Implementation Tasks

- [ ] 1. apply outcome を `BLOCKED` と `REJECT_PROPOSAL` に分離する canonical contract を spec と prompt に追加する (verification: integration - `openspec/specs/agent-prompts/spec.md` と `openspec/specs/parallel-execution/spec.md` に apply-generated blocker / rejection proposal の別経路が定義され、`cflx openspec validate separate-apply-block-from-reject --strict --evidence warn` が成功する)
- [ ] 2. runtime state / event / workspace status に resume-capable `Blocked` activity を追加し、worktree / WIP / task progress / blocker metadata を保持する遷移を定義する (verification: unit - reducer/state scenarios prove `Applying -> Blocked`, `Rejecting -> Blocked`, `Blocked -> Applying` and distinguish `Blocked` from terminal `Rejected`)
- [ ] 3. apply runtime が recoverable blocker では `REJECTED.md` を生成せず blocked handoff を発火し、terminal rejection proposal でのみ dedicated rejecting flow に入るよう実装する (verification: integration - add/update orchestration tests covering `src/execution/apply.rs`, `src/execution/state.rs`, and `src/parallel/dispatch.rs` so apply-generated `BLOCKED` handoff and apply-generated `REJECTED.md` handoff are asserted separately)
- [ ] 4. rejecting review verdict を `CONFIRM` / `RESUME` / `BLOCK` に拡張し、reject proposal 不採用時に即 apply 再開すべきケースと blocked 保留すべきケースを分離する (verification: integration - add/update rejecting review routing tests under `src/parallel/dispatch.rs` / `src/orchestration/rejection.rs` proving `REJECTION_REVIEW: BLOCK` returns the change to `Blocked` with worktree preserved)
- [ ] 5. TUI / Web / server state surfaces が `Blocked` を read-write resumable stop state として表示し、`Rejected` terminal row や `Applying` active rowと混同しないことを確認する (verification: unit/integration - add/update state mapping tests covering `src/server/api/ws.rs`, `src/web/state.rs`, and TUI state snapshot assertions for blocked/rejected/applying rows)
- [ ] 6. proposal delta と関連実装変更を strict validation、Rust tests、lint で確認する (verification: integration - run `cflx openspec validate separate-apply-block-from-reject --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`, including tests covering `src/execution/apply.rs`, `src/execution/state.rs`, and `src/parallel/dispatch.rs`)

## Future Work

- blocked reason の自動 dedupe / repeated blocker loop suppression policy を追加する
- blocked changes 向けの operator UX（resume with note / unblock reason editing / human handoff dashboard）を強化する
