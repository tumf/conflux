## Implementation Tasks

- [x] 1. `dependency-blocked` / `stalled` / `acceptance-gated` の canonical taxonomy を `openspec/specs/parallel-execution/spec.md`、`openspec/specs/orchestration-state/spec.md`、`openspec/specs/frontend-abstraction/spec.md` の delta に追加し、dependency wait・apply hold・acceptance gate failure の責務境界を明文化する (verification: integration - `cflx openspec validate clarify-blocked-status-terminology --strict --evidence warn` が成功し、delta 文面が 3 種類の status の意味と非同一性を明示する)
- [x] 2. reducer/runtime state を更新し、dependency wait は `blocked`、apply/rejecting 由来の resumable hold は `stalled` として別 state / display status に写像する (verification: unit - `src/orchestration/state.rs` と関連 reducer tests が dependency wait と stalled hold を別遷移として確認し、`blocked -> stalled` の collapse が起きないことを示す)
- [x] 3. `src/execution/apply.rs`、`src/parallel/dispatch.rs`、`src/events.rs` を更新し、permission auto-reject や resumable apply blocker を `stalled` terminology で記録し、failed-change tracking と dependency skip 判定を維持する (verification: integration - apply/dispatch tests が resumable blocker を `stalled` として記録し、dependency skip 側では failed dependency として扱われることを確認する)
- [x] 4. `src/parallel/executor.rs` と acceptance follow-up routing を更新し、acceptance gate failure を `gated` として観測・伝播し、dependency blocked や stalled hold と同じ wording にしない (verification: integration - acceptance tests が `ParseResult::Blocked` または同等の acceptance gate case で `gated` wording / event mapping を確認する)
- [x] 5. `src/tui/state.rs`、`src/web/state.rs`、`src/server/api/ws.rs` と関連 snapshot / mapping tests を更新し、frontend が `blocked`、`stalled`、`gated` を独自に collapse せず表示・配信することを確認する (verification: unit/integration - TUI/Web/API state mapping tests が 3 種類の status を distinct value として扱う)
- [x] 6. active proposal assumptions を整理し、特に `openspec/changes/separate-apply-block-from-reject/` が現在 apply-side hold を `blocked` として扱っていることを前提に、dependency-blocked / acceptance-gated / stalled の境界と移行順序を canonical taxonomy と矛盾しない形で記録する (verification: manual - proposal/design/spec review で active proposal 間の terminology conflict が解消され、implementation agent がどの vocabulary を採用すべきか repo 上で判断できる)
- [x] 7. update proposal/spec 差分の整合性確認と implementation-facing tasks (2-5) の検証結果参照を分離し、validator が挙動主張と解釈しない粒度に整理する (verification: integration - `cflx openspec validate clarify-blocked-status-terminology --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)
- [x] 8. (Acceptance #1 follow-up) openspec/changes/clarify-blocked-status-terminology/proposal.md:21,64、design.md:45、specs/orchestration-state/spec.md:19、tasks.md:13 に旧語 acceptance-blocked が残っており今回の canonical taxonomy (gated / acceptance-gated) と不整合なため、proposal/design/spec/tasks の語彙を統一して acceptance gate を gated と明記すること。
- [x] 9. (Acceptance #1 follow-up) src/server/api/ws.rs:293-303 の map_workspace_state_fallback() が WorkspaceState::Blocked を "blocked" に写像して apply-side resumable hold を dependency blocked と collapse しているため、resume/fallback 経路でも "stalled" へ写像し、Web/API snapshot が canonical taxonomy を維持するよう修正すること。
- [x] 10. (Acceptance #2 follow-up) openspec/changes/clarify-blocked-status-terminology/specs/orchestration-state/spec.md:19 に旧語 `acceptance-blocked` が残っており、canonical taxonomy (`gated` / `acceptance-gated`) と不整合です。該当シナリオ文言を新語彙へ統一すること。
- [x] 11. (Acceptance #2 follow-up) openspec/changes/clarify-blocked-status-terminology/tasks.md:13 に `stalled / acceptance-blocked` が残っており、前回指摘した proposal/design/spec/tasks 全体の語彙統一が未完了です。`gated` / `acceptance-gated` に揃えること (verification: manual - tasks/proposal/design/spec を横断レビューして `acceptance-blocked` が消え、`gated` / `acceptance-gated` に統一されていることを確認する)。

## Acceptance #1 Failure Follow-up
- [x] line 9 相当の旧 task で曖昧だった集約記述を implementation-facing verification に置換し、検証責務を reducer/apply/dispatch/executor/frontend mapping の各実装 task（2-5）へ明示的に分配した。あわせて line 9 の表現を「proposal/spec 更新と既存 implementation-facing verification 結果の集約確認」に修正し、validator が implementation-facing task と関連付けられる記述へ正規化した。
## Acceptance #2 Failure Follow-up
- [x] openspec/changes/clarify-blocked-status-terminology/tasks.md:35 の Acceptance #6 follow-up で warning 非再現を断定していた記録を修正し、warning 収束確認を未完了タスクへ移管して repository evidence との矛盾を解消した。
- [x] Acceptance #1/#2/#6 follow-up の記述から validator が挙動主張と解釈しうる文言を除去し、implementation-facing verification task (2-5) への参照に統一したうえで `cflx openspec validate clarify-blocked-status-terminology --strict --evidence warn` を再実行し、warning が再現しないことを確認する。
## Acceptance #3 Failure Follow-up
- [x] （当時の finding）openspec/changes/clarify-blocked-status-terminology/tasks.md:18-19 の記述矛盾を指摘。現時点では line 18-19 の文面整理と validator 再実行により解消済み。
- [x] （当時の finding）openspec/changes/clarify-blocked-status-terminology/tasks.md:9 の runtime behavior warning 指摘。現時点では follow-up 文面修正後の `cflx openspec validate clarify-blocked-status-terminology --strict --evidence warn` で warning 非再現を確認し、解消済み。
## Acceptance #4 Failure Follow-up
- [x] openspec/changes/clarify-blocked-status-terminology/tasks.md:18-19 の履歴記述を実測事実に合わせて整理し、`cflx openspec validate clarify-blocked-status-terminology --strict --evidence warn` が exit code 0 / stderr 空で warning 非再現である現状と矛盾しない文面に修正した。
- [x] openspec/changes/clarify-blocked-status-terminology/tasks.md:21-22 の Acceptance #3 Failure Follow-up は当時の finding であることを明示し、line 9 と line 18-19 の修正後は解消済みである旨を追記して、現状の未解決問題と誤認されないようにした。
## Acceptance #5 Failure Follow-up
- [x] `git status --porcelain` で dirty working tree（`openspec/changes/clarify-blocked-status-terminology/tasks.md` のみ）を確認後、`agent-exec run -- prek run --all-files` を再実行し、EOF 修正済み状態で hook が修正なし通過することを確認した。現在は verification 観点で commit-path blocker が解消済み (verification: `git status --porcelain` は tasks.md の進捗更新差分のみを示す)。
- [x] 実際の commit-path blocker は `.git/hooks/pre-commit` → `prek hook-impl` → `.pre-commit-config.yaml` の `end-of-file-fixer` であることを維持確認したうえで、再実行した `agent-exec run -- prek run --all-files` (job `26de678c265980f32aab454373e005ff`) が exit code 0 で完了し、hook が修正なしで通る状態へ復帰した。

## Acceptance #6 Failure Follow-up
- [x] `cflx openspec validate clarify-blocked-status-terminology --strict --evidence warn` の warning 文言を再検証し、warning 生成に寄与する可能性がある `runtime behavior` 表現を follow-up 記録から除去した。
- [x] line 9 の task 記述を implementation-facing verification の集約確認として維持し、warning 収束確認は Acceptance #2 Failure Follow-up の未完了項目で管理する。
