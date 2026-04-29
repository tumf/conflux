## Implementation Tasks

- [x] 1. `dependency-blocked` / `stalled` / `acceptance-gated` の canonical taxonomy を `openspec/specs/parallel-execution/spec.md`、`openspec/specs/orchestration-state/spec.md`、`openspec/specs/frontend-abstraction/spec.md` の delta に追加し、dependency wait・apply hold・acceptance gate failure の責務境界を明文化する (verification: integration - `cflx openspec validate clarify-blocked-status-terminology --strict --evidence warn` が成功し、delta 文面が 3 種類の status の意味と非同一性を明示する)
- [x] 2. reducer/runtime state を更新し、dependency wait は `blocked`、apply/rejecting 由来の resumable hold は `stalled` として別 state / display status に写像する (verification: unit - `src/orchestration/state.rs` と関連 reducer tests が dependency wait と stalled hold を別遷移として確認し、`blocked -> stalled` の collapse が起きないことを示す)
- [x] 3. `src/execution/apply.rs`、`src/parallel/dispatch.rs`、`src/events.rs` を更新し、permission auto-reject や resumable apply blocker を `stalled` terminology で記録し、failed-change tracking と dependency skip 判定を維持する (verification: integration - apply/dispatch tests が resumable blocker を `stalled` として記録し、dependency skip 側では failed dependency として扱われることを確認する)
- [x] 4. `src/parallel/executor.rs` と acceptance follow-up routing を更新し、acceptance gate failure を `gated` として観測・伝播し、dependency blocked や stalled hold と同じ wording にしない (verification: integration - acceptance tests が `ParseResult::Blocked` または同等の acceptance gate case で `gated` wording / event mapping を確認する)
- [x] 5. `src/tui/state.rs`、`src/web/state.rs`、`src/server/api/ws.rs` と関連 snapshot / mapping tests を更新し、frontend が `blocked`、`stalled`、`gated` を独自に collapse せず表示・配信することを確認する (verification: unit/integration - TUI/Web/API state mapping tests が 3 種類の status を distinct value として扱う)
- [x] 6. active proposal assumptions を整理し、特に `openspec/changes/separate-apply-block-from-reject/` が現在 apply-side hold を `blocked` として扱っていることを前提に、dependency-blocked / acceptance-gated / stalled の境界と移行順序を canonical taxonomy と矛盾しない形で記録する (verification: manual - proposal/design/spec review で active proposal 間の terminology conflict が解消され、implementation agent がどの vocabulary を採用すべきか repo 上で判断できる)
- [x] 7. proposal delta と関連実装変更をまとめて検証する (verification: integration - `cflx openspec validate clarify-blocked-status-terminology --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- stalled / acceptance-gated の operator UX（resume action、reason editing、badge tooltip）を dashboard / TUI で磨く
- archive/readiness/external unblock を含む broader blocker taxonomy を別 change で再整理する

## Acceptance #1 Failure Follow-up
- [x] openspec/changes/clarify-blocked-status-terminology/proposal.md:21,64、design.md:45、specs/orchestration-state/spec.md:19、tasks.md:13 に旧語 acceptance-blocked が残っており今回の canonical taxonomy (gated / acceptance-gated) と不整合なため、proposal/design/spec/tasks の語彙を統一して acceptance gate を gated と明記すること。
- [x] src/server/api/ws.rs:293-303 の map_workspace_state_fallback() が WorkspaceState::Blocked を "blocked" に写像して apply-side resumable hold を dependency blocked と collapse しているため、resume/fallback 経路でも "stalled" へ写像し、Web/API snapshot が canonical taxonomy を維持するよう修正すること。

## Acceptance #2 Failure Follow-up
- [x] openspec/changes/clarify-blocked-status-terminology/specs/orchestration-state/spec.md:19 に旧語 `acceptance-blocked` が残っており、canonical taxonomy (`gated` / `acceptance-gated`) と不整合です。該当シナリオ文言を新語彙へ統一してください。
- [x] openspec/changes/clarify-blocked-status-terminology/tasks.md:13 に `stalled / acceptance-blocked` が残っており、前回指摘した proposal/design/spec/tasks 全体の語彙統一が未完了です。future work を含め `gated` / `acceptance-gated` に揃えてください。
