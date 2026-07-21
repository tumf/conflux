## Implementation Tasks

- [x] 1. Task parserを`Current Acceptance Follow-up` single-section upsertへ変更し、legacy numbered runtime sectionsを次回updateで置換する。(verification: unit - `cargo test task_parser`でactive/archive path、single section、obsolete removal、unknown section preservationを検証; completion condition:runtime-owned follow-upが最大1件)
- [x] 2. Stable finding identityでsort/dedupし、最新FAILで再報告されたrepository findingを必ずuncheckedへ戻し、external blockersをnon-checkbox metadataへrenderする。(verification: unit - `cargo test task_parser`でsame-code detail changeのreopen、mixed scopes、external-only、evidence/next actionを検証; completion condition:再報告findingがcompleted taskとして残らない)
- [x] 3. Acceptance prompt builderをcurrent diffとlatest normalized findings一回へ縮小し、finalized FAIL時のraw/history重複を除去する。(verification: unit - `cargo test history && cargo test agent::prompt`で3+ attempt history、旧findings/output不在、latest重複不在を検証; completion condition:all-attempt historyがprompt inputにならない)
- [x] 4. CONTINUE、finding-less verdict、command diagnostics向けbounded latest raw-output fallbackを維持する。(verification: unit - `cargo test history && cargo test agent::prompt`で各fallbackとsize boundを検証; completion condition:diagnostic contextを全削除しない)
- [x] 5. Serial/parallelを同じfollow-up rendererとlatest-context builderへ接続する。(verification: integration - `cargo test parallel::dispatch && cargo test serial_run_service`でequivalent fixturesのtasks/prompt shapeを検証; completion condition:mode固有history replayが残らない)
- [x] 6. Canonical agent promptとbundled acceptance guidanceをread-only/runtime-owned contractへ同期する。(verification: unit - `cargo test embedded_skills`でtask edit instruction不在、current section ownership、mixed blocker guidanceを検証; completion condition:authoritative guidance間に矛盾がない)
- [x] 7. Targeted regressionとquality gatesを実行する。(verification: integration - `cargo test task_parser && cargo test history && cargo test agent::prompt && cargo test parallel::dispatch && cargo test serial_run_service && cargo test embedded_skills && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`; completion condition:全commandが成功する)

## Future Work

- Native structured-finding-only protocolは全runtime移行後に検討する。

## Final Validation

Expected archive gate: `cflx openspec validate compact-acceptance-retry-context --archive-gate` exits 0.

## Acceptance #2 Failure Follow-up
- [x] [canonical_scenario_contradiction] `openspec/specs/agent-prompts/spec.md:86-91` の既存 Scenario は、read-only/runtime-owned に変更した Requirement と矛盾し、引き続き acceptance agent に `tasks.md` の follow-up 追記手順を要求している。Scenario を runtime が follow-up を永続化し、agent は `tasks.md` を編集しない内容へ更新すること。
- [x] [external_metadata_lost_on_resume] `src/task_parser.rs:751-783` の `read_acceptance_follow_up` は checkbox finding だけを復元し、`### External blockers` metadata を読み戻さない。さらに `src/execution/apply.rs:125-151` の hydrate/ensure 経路が復元した repository findings だけで section を再生成するため、mixed finding の external evidence/next action が再開時に消える。external metadata を構造化して復元・再描画し、restart/resume の回帰テストを追加すること。
