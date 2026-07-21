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

## Acceptance #5 Failure Follow-up
- [x] 作業ツリーが未コミットです。`git status --short` は `openspec/changes/persist-acceptance-stalled-state/tasks.md` を `M` と報告しています。前回指摘のチェックボックスは削除され、全active taskは `[x]`、`cflx openspec validate persist-acceptance-stalled-state --archive-gate` と宣言済みCargoテスト・fmt・clippyは成功していますが、acceptance規約上dirty working treeではPASSにできません。`tasks.md:19` の空の `## Acceptance #4 Failure Follow-up` 見出しも除去し、必要なtasks.md修正をコミットして作業ツリーをcleanにしてください。
