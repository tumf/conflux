## Implementation Tasks

- [x] 1. Existing workspace state contractへnon-blocking acceptance retry checkpointを追加し、previous finding identities、semantic fingerprint、cycle countをatomicにwrite/readする。(verification: unit - `cargo test execution::state`でround-trip、missing、malformed、atomic write failureを検証; completion condition:初回FAIL後とapply後のrestartで同じretry contextを復元する)
- [x] 2. Existing `APPLY_BLOCKED/marker.md`にacceptance originとstructured stalled evidenceを追加し、legacy/unknown markerを保守的にparseする。(verification: unit - `cargo test execution::state`でround-trip、legacy、unknown origin、malformed inputを検証; completion condition:parserがreason、evidence、resumability、next actionを復元する)
- [x] 3. Acceptance-generated marker writerを実装し、checkpoint evidenceを引き継ぎ、atomic write failureをworkflow errorとして扱う。(verification: unit - `cargo test execution::state`でcheckpoint-to-marker migration、write/read、write failureを検証; completion condition:partial markerをrouting evidenceとして残さない)
- [x] 4. Workspace scanとordinary parallel dispatchをcheckpoint/marker-backed routingへ接続する。(verification: integration - `cargo test parallel::dispatch`でruntime stateなしのpre-stall/stalled restart fixture、apply/acceptance/archive未実行、stalled metadataを検証; completion condition:out-of-worktree state削除後もdecisionが同じ)
- [x] 5. Parallel explicit retryへorigin/resumability-aware marker consumeを接続する。(verification: integration - `cargo test parallel::dispatch`でacceptance marker consume、apply/unknown/non-resumable marker保持、consume failure時dispatch停止を検証; completion condition:reducer clearだけでretry成功扱いしない)
- [x] 6. Serial preflight、ordinary routing、explicit retryへ同じcheckpoint/marker writer、scanner、consumerを接続する。(verification: integration - `cargo test serial_run_service`でpre-stall restart、stalled restart、archive suppression、acceptance marker consume、foreign marker preservationを検証; completion condition:serialがmarker存在中にapply/acceptance/archiveへ進まない)
- [x] 7. Targeted regressionとquality gatesを実行する。(verification: integration - `cargo test execution::state && cargo test orchestration::state && cargo test parallel::dispatch && cargo test serial_run_service && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`; completion condition:全commandが成功する)

## Future Work

- Repeated findingとcycle ceilingからmarker writerを呼ぶpolicy wiringは`bound-acceptance-retry-cycles`で実装する。

## Final Validation

Expected archive gate: `cflx openspec validate persist-acceptance-stalled-state --archive-gate` exits 0.

## Acceptance #1 Failure Follow-up
- [ ] Checkpoint が未 ignore の workspace-root ACCEPTANCE_STATE.json に書かれる（src/parallel/acceptance_state.rs:10,73-75、.gitignore:9）。archive は src/execution/archive.rs:365-370 で git add -A するため runtime state を commit し得て、その後 src/parallel/executor.rs:1175-1180 が削除して dirty deletion を残す。archive commit 前に checkpoint を除外または安全に消去すること。
- [ ] GATED の stalled evidence が永続化されない。serial は src/serial_run_service.rs:553-560 で marker writer を迂回し、parallel は src/parallel/dispatch.rs:1967-2019 で reducer event のみ送る。両経路で structured acceptance marker を書き、restart 復元テストを追加すること。
- [ ] Malformed marker が fail-open する。src/execution/state.rs:438-445 の parse error を src/parallel/dispatch.rs:697-713 が WorkspaceState::Created/Apply に変換する。state detection error を workflow error として dispatch 前に停止し、marker を保持すること。
- [ ] Marker schema の必須 evidence が実値を保持しない。src/parallel/acceptance_state.rs:252-265 は semantic_progress を常に stalled、external_blockers を常に空にする。実際の semantic progress と retained external blockers を writer API へ渡すこと。
- [ ] Parallel explicit retry が対象 worktree ではなく base repo の marker を consume する。src/tui/orchestrator.rs:1007-1015 は workspace 発見前に repo_root を渡すが、実 workspace は src/parallel/dispatch.rs:631-639 で再利用される。workspace 解決後に consume し、acceptance/apply/unknown/non-resumable/consume failure を検証すること。
- [ ] Pre-stall checkpoint の previous finding identities と semantic fingerprint が routing/decision に復元されない。src/parallel/dispatch.rs:872-877 は cycle_count だけを読み、load error も破棄する。serial の retry count も src/serial_run_service.rs:746-760 で process-local AgentRunner から再計算される。checkpoint 全項目を復元し、restart 後の次回 FAIL 判定を検証すること。
- [ ] Restart 時に stalled metadata が復元されない。src/execution/state.rs:438-445 は parsed marker を破棄し、src/parallel/dispatch.rs:747-763 は metadata のない Blocked status のみ送る。reason、evidence、resumability、next action を reducer/display state へ復元すること。
- [ ] tasks.md:3-9 は全完了だが、宣言した atomic write failure、parallel restart/explicit consume/foreign preservation/consume failure、serial pre-stall restart/consume failure のテスト証拠が不足する。実装修正後、宣言どおりの unit/integration tests を追加すること。Quality gates、OpenSpec strict/archive-gate、pre-commit hook は成功し、作業ツリーは clean、未チェック task はない。
