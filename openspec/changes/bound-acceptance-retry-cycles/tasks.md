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

## Acceptance #2 Failure Follow-up
- [x] `openspec/changes/bound-acceptance-retry-cycles/tasks.md:3-9` は全項目を完了扱いにしているが、宣言された alternating CONTINUE/FAIL、parallel の repeated-finding/cycle-limit、apply回数、Error event不在、serial/parallel outcome parity のテストが実装されていない。前回要求された parallel follow-up 回帰も `src/orchestration/acceptance.rs:597-604` の共有helperテストと `src/serial_run_service.rs:1301-1410` のserialテストだけで、parallel dispatch経路を実証していない。
- [x] `src/orchestration/acceptance.rs:32-40` は `credential`、`api key`、`network`、`unavailable` などの単語だけで external/non-mockable と分類するため、mock可能な資格情報不足や repository 内の unavailable エラーも external stalled になり得る。spec が要求する repository-fixable と external/non-mockable の区別を実装し、誤分類の回帰テストを追加すること。
- [x] `src/orchestration/acceptance.rs:97-108` の semantic fingerprint は `config/`、`openspec/specs/`、`Cargo.toml` など限定されたパスだけを対象にしており、change 固有の spec delta `openspec/changes/<id>/specs/**` や `.cflx.jsonc` 等の実設定変更を除外する。spec の「configuration、spec の substantive change」を進捗として扱う要件を満たすよう対象を修正し、各パスのテストを追加すること。
- [x] tasks.md:9 の必須ゲート `cargo test parallel::tests::executor` は再実行しても exit 101。`src/parallel/tests/executor.rs:492`、`:694`、`:3011` の3件（`resolving_dependency_blocks_its_dependent_but_not_unrelated_dispatch`、`test_dependency_on_terminal_error_is_blocked_until_retry_and_success`、`test_resolve_wait_completion_unblocks_dependents`）が失敗する。チェック済み表示を維持するにはゲートを成功させること。なお実コミットフック `prek run --all-files` と `cflx openspec validate bound-acceptance-retry-cycles --archive-gate` は成功し、worktreeもclean。
