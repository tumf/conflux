## Implementation Tasks

- [ ] 1. acceptance follow-up を apply-driving remediation と blocker-only finding に分ける canonical contract を `openspec/specs/parallel-execution/spec.md` と `openspec/specs/agent-prompts/spec.md` の delta に追加する (verification: integration - `cflx openspec validate classify-acceptance-followup-routing --strict --evidence warn` が成功し、delta 文面が follow-up format と routing behavior の両方を明示する)
- [ ] 2. `src/task_parser.rs` に section-aware follow-up parser を追加し、`Implementation Tasks`・apply-driving follow-up checkbox・blocker-only follow-up note を区別して apply-routing 用 progress を算出できるようにする (verification: unit - task parser tests が `Acceptance #1 Failure Follow-up` 内の remediation checkbox と blocker note を別カテゴリとして数え、blocker note が raw progress に混ざらないことを確認する)
- [ ] 3. `src/parallel/dispatch.rs` の resume routing と acceptance-fail reroute を更新し、blocker-only follow-up だけが残る `Applied` workspace を `Apply` へ戻さず blocked/non-apply path に送る (verification: integration - dispatch/resume tests が `Implementation Tasks` 完了 + blocker-only follow-up の workspace で `Apply` を選ばず、implementation remediation follow-up を含む workspace では `Apply` を選ぶことを確認する)
- [ ] 4. `src/serial_run_service.rs` と acceptance follow-up 記録箇所を更新し、commit-path / archive-readiness blocker を checkbox remediation task と同じ書式で残さない canonical formatting を実装する (verification: unit - acceptance follow-up recording tests が remediation checkbox と blocker note の両方を生成し、同一 failure section でも parser が分類可能な形式になっていることを確認する)
- [ ] 5. empty WIP stall regression を追加し、`add-running-agents-restart-button` 型の「implementation complete + archive blocker only」ケースで apply rerun による空 WIP stall が再発しないことを確認する (verification: integration - `src/parallel/tests/executor.rs` または同等の test で blocker-only follow-up case が stall error ではなく blocked/non-apply outcome になることを確認する)
- [ ] 6. log / event wording と関連コードコメントを更新し、`implementation tasks incomplete` と `blocker-only follow-up remains`、および temporary blocker と terminal rejection の判断基準を区別して観測・保守できるようにする (verification: unit - state/log assertion tests が resumed workspace reason message を新 wording で確認し、コードコメントが `Blocked` vs `Rejected` の使い分けを明示していることをレビューする)
- [ ] 7. disk exhaustion・ローカル検証不能・commit-path blocker のような再開可能 blocker を `Blocked` として扱い、change 前提破綻や superseded closure のみを `Rejected` とする canonical rule を spec / design / prompt delta に追加する (verification: integration - spec delta に temporary blocker vs terminal rejection scenario があり、`cflx openspec validate classify-acceptance-followup-routing --strict --evidence warn` が成功する)
- [ ] 8. proposal delta と実装変更をまとめて検証する (verification: integration - `cflx openspec validate classify-acceptance-followup-routing --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- acceptance follow-up の blocker kind を UI から編集・解除できる operator UX を追加する
- archive readiness blocker と external human-unblock blocker を別 status badge で見せる dashboard 改善を検討する
