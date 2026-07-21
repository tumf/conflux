## Implementation Tasks

- [x] 1. Shared deterministic finding normalization、identity comparison、repository/external scope classificationを実装する。(verification: unit - `cargo test orchestration::acceptance`でorder、whitespace、duplicates、stable code/path、legacy strings、mixed scopes、finding-less FAILを検証; completion condition:serial/parallelが同じnormalized representationを使用する)
- [x] 2. Committed/uncommittedのsource/test/config/spec/substantive-taskを含み、runtime-managed follow-up、marker、counter、logs、UI/historyを除外するsemantic progress fingerprintを実装する。(verification: unit - `cargo test orchestration::acceptance`でsource/spec/task変更とbookkeeping-only変更を区別する; completion condition:同一repository semanticsが同じfingerprintになる)
- [x] 3. 初回FAIL後のapply、同一finding/no-progress stall、progress/changed-finding retry、mixed external blocker保持を決めるshared retry decisionを実装する。(verification: unit - `cargo test orchestration::acceptance`で各分岐とalternating CONTINUE/FAIL fixtureを検証; completion condition:decisionがreasonとevidenceを返す)
- [x] 4. Shared decisionをworkspace-local retry checkpointのload/updateへ接続し、restartでfirst-attempt扱いやcycle resetへ戻らないようにする。(verification: integration - `cargo test parallel::dispatch && cargo test serial_run_service`で初回FAIL後、apply後、次回acceptance前のrestartを検証; completion condition:restart前後でdecision inputとcycle countが同じ)
- [x] 5. Parallel dispatchへshared decisionを接続し、cycle 10をterminal Errorではなく`acceptance_cycle_limit_exhausted` marker-backed stalledへ変える。(verification: integration - `cargo test parallel::dispatch && cargo test parallel::tests::executor`でapply回数、Error event不在、marker evidenceを検証; completion condition:ordinary loopがstalled後に再dispatchしない)
- [x] 6. Serial executionへ同じdecision、10-cycle ceiling、marker-backed stalled handoffを接続する。(verification: integration - `cargo test serial_run_service`でparallel相当fixture、cycle exhaustion、outcome parityを検証; completion condition:serialが無制限またはterminal-error-only FAIL loopへ入らない)
- [x] 7. Affected regressionとquality gatesを実行する。(verification: integration - `cargo test orchestration::acceptance && cargo test parallel::dispatch && cargo test parallel::tests::executor && cargo test serial_run_service && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`; completion condition:各commandが成功し、1秒超testはheavy扱いになる)

## Future Work

- `acceptance_max_continues`のcanonical/default不一致はfocused configuration changeで扱う。

## Final Validation

Expected archive gate: `cflx openspec validate bound-acceptance-retry-cycles --archive-gate` exits 0.

## Acceptance #3 Failure Follow-up
- [x] `openspec/changes/bound-acceptance-retry-cycles/tasks.md:3-8` は全項目を完了扱いにしているが、task 3 の alternating CONTINUE/FAIL fixture、および task 6 の serial/parallel outcome parity 検証が実装されていない。`src/orchestration/acceptance.rs:552-675`、`src/serial_run_service.rs:1301-1421`、`src/parallel/tests/executor.rs:6489-6663` には個別分岐テストはあるが、宣言された交互verdictと同一入力のmode parityを実証するテストがない。チェック済みの検証契約を満たす回帰テストを追加するか、tasks.mdの完了表示を実証可能な内容へ修正すること。
- [x] `openspec/changes/bound-acceptance-retry-cycles/tasks.md:9` は品質ゲートを完了済みにしているが、宣言されたコマンド列は失敗する。クリーン再ビルド後の `cargo test serial_run_service` で `src/serial_run_service.rs:1348-1369` の `serial_external_only_failure_stalls_without_apply_findings` が失敗した。fixture は `network unavailable` を external と期待する一方、`src/orchestration/acceptance.rs:34-43` の classifier はこの文字列を repository-fixable と分類する。前回修正方針に合わせ、external/non-mockable を明示するfixtureへ直すか、意図した分類契約と実装を一致させてゲートを成功させること。
