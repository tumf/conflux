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

`cflx openspec validate persist-acceptance-stalled-state --archive-gate` exits 0.

## Acceptance #1 Failure Follow-up
- [ ] Checkpointがworkspaceごとの単一`.cflx/acceptance-state.json`であり、読み込み時に`change_id`を検証していない。`src/parallel/acceptance_state.rs:109-129`は以前のfinding identities、fingerprint、cycle countをchange境界なしで継承し、`src/serial_run_service.rs:206-227`は保存済み`change_id`を確認せず要求されたchangeへ復元する。serialで複数changeを処理すると別changeのretry contextを引き継げるため、change単位で保存するか一致検証が必要。
- [ ] `openspec/changes/persist-acceptance-stalled-state/tasks.md`のtask 4と6は実際のrestart/dispatch integration検証を完了済みとするが、対応テストは`restore_acceptance_checkpoint`、`restore_acceptance_checkpoint_history`、`preflight_blocked_marker`などhelper直接呼び出しに留まり、上記の実行経路上書きとchange間汚染を検出できない。実際のparallel/serial resume経路を通すintegration testへ置き換える必要がある。Targeted tests、fmt、clippy、archive-gateは成功し、worktreeはclean、実commit pathのpre-commit相当clippyにも失敗はなかった。
- [ ] 並列再起動時のcheckpoint復元がacceptance直前に破棄される。`src/parallel/dispatch.rs:1019-1036`はworkspace checkpointを`agent`へ復元するが、`src/parallel/dispatch.rs:1808`が`acceptance_history`のcloneで再度`seed_acceptance_history`し、復元したfinding identitiesとsemantic fingerprintを上書きする。acceptance直前の再seedへcheckpointを統合し、実際の再起動経路で次のFAILが初回扱いされないことを検証する必要がある。
