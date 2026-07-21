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

## Acceptance #3 Failure Follow-up
- [x] [dirty_worktree] `git status --porcelain=v1` が `MM openspec/changes/compact-acceptance-retry-context/tasks.md` を報告している。index と working tree の内容が不一致であり、現状の archive commit では未コミット差分が残る。最終内容を確認して同ファイルを再stageし、`git status --porcelain=v1` を空にすること。なお archive gate、対象テスト、`cargo fmt --check`、`cargo clippy --locked --all-targets --all-features -- -D warnings` は成功済み。
