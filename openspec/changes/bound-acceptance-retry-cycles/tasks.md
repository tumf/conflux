## Implementation Tasks

- [x] 1. Shared deterministic finding normalization、identity comparison、repository/external scope classificationを実装する。(verification: unit - `cargo test orchestration::acceptance`でorder、whitespace、duplicates、stable code/path、legacy strings、mixed scopes、finding-less FAILを検証; completion condition:serial/parallelが同じnormalized representationを使用する)
- [x] 2. Committed/uncommittedのsource/test/config/spec/substantive-taskを含み、runtime-managed follow-up、marker、counter、logs、UI/historyを除外するsemantic progress fingerprintを実装する。(verification: unit - `cargo test orchestration::acceptance`でsource/spec/task変更とbookkeeping-only変更を区別する; completion condition:同一repository semanticsが同じfingerprintになる)
- [x] 3. 初回FAIL後のapply、同一finding/no-progress stall、progress/changed-finding retry、mixed external blocker保持を決めるshared retry decisionを実装する。(verification: unit - `cargo test orchestration::acceptance`で各分岐とalternating CONTINUE/FAIL fixtureを検証; completion condition:decisionがreasonとevidenceを返す)
- [x] 4. Shared decisionをworkspace-local retry checkpointのload/updateへ接続し、restartでfirst-attempt扱いやcycle resetへ戻らないようにする。(verification: integration - `cargo test parallel::dispatch && cargo test serial_run_service`で初回FAIL後、apply後、次回acceptance前のrestartを検証; completion condition:restart前後でdecision inputとcycle countが同じ)
- [x] 5. Parallel dispatchへshared decisionを接続し、cycle 10をterminal Errorではなく`acceptance_cycle_limit_exhausted` marker-backed stalledへ変える。(verification: integration - `cargo test parallel::dispatch && cargo test --features heavy-tests parallel::tests::executor`でapply回数、Error event不在、marker evidenceを検証; completion condition:ordinary loopがstalled後に再dispatchしない)
- [x] 6. Serial executionへ同じdecision、10-cycle ceiling、marker-backed stalled handoffを接続する。(verification: integration - `cargo test serial_run_service`でparallel相当fixture、cycle exhaustion、outcome parityを検証; completion condition:serialが無制限またはterminal-error-only FAIL loopへ入らない)
- [x] 7. Affected regressionとquality gatesを実行する。(verification: integration - `cargo test orchestration::acceptance && cargo test parallel::dispatch && cargo test --features heavy-tests parallel::tests::executor && cargo test serial_run_service && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`; completion condition:各commandが成功し、1秒超testはheavy扱いになる)

## Future Work

- `acceptance_max_continues`のcanonical/default不一致はfocused configuration changeで扱う。

## Final Validation

Expected archive gate: `cflx openspec validate bound-acceptance-retry-cycles --archive-gate` exits 0.

## Acceptance #2 Failure Follow-up
- [x] 修正コミット `fc21ad08` は実装・テストコードを変更せず、`target-acceptance/` 配下の生成物6053ファイルを追跡追加している。前回findingへの実質的な修正になっておらず、ビルド成果物をrepositoryから削除してignoreし、必要なコード修正だけをコミットすること。
- [x] 品質ゲート未達。`src/parallel/tests/mod.rs:8` の executor tests は `heavy-tests` featureなしでは実行されず、`cargo test parallel::tests::executor` は0件実行で成功するだけ。一方、実テストを走らせる `cargo test --features heavy-tests parallel::tests::executor` は exit 101 で、`src/parallel/tests/executor.rs:5880` の `test_manual_resolve_wait_retries_after_in_flight_apply_completes`、同ファイルの `test_merge_proceeds_when_archive_complete`、`test_resolve_merge_aborts_when_base_dirty` が失敗した。失敗を修正し、`tasks.md:7,9` の検証コマンドを実際に対象テストが走る形へ訂正すること。
