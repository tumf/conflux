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

## Acceptance #1 Failure Follow-up
- [x] `openspec/changes/bound-acceptance-retry-cycles/tasks.md:3-9` は全項目を完了扱いにしているが、宣言された mixed scopes、finding-less FAIL、alternating CONTINUE/FAIL、parallel apply 回数・Error event 不在の検証が実装 diff にない。`src/orchestration/acceptance.rs:513-564` の追加 unit test は bookkeeping、重複正規化、単一 external finding、cycle limit のみを検証している。計画済み verification を追加するか、tasks.md の完了表示を実証可能な内容へ修正すること。
- [x] `src/orchestration/acceptance.rs:155-163` は repository finding の有無を判定せず、external-only の初回または finding 集合変化を Retry にする。さらに `src/parallel/dispatch.rs:1983-1987` と `src/serial_run_service.rs:878-883` は external finding も follow-up に書き込み、`src/agent/prompt.rs:434` は apply に全 finding の修正を指示する。これは spec の external-only stalled 保持と『apply は external prerequisite を repository edit で満たすよう指示されない』要件に反する。repository findings のみ apply 入力へ渡し、external-only は resumable stalled にすること。
- [x] `src/parallel/dispatch.rs:1983-2044` と `src/serial_run_service.rs:873-878` は runtime-owned acceptance follow-up を `tasks.md` に書き込んだ後で semantic fingerprint を計算する一方、`src/orchestration/acceptance.rs:95-96` は全 `tasks.md` 内容を fingerprint 対象にしている。attempt ごとの follow-up heading/content 更新が semantic progress と誤認され、同一 finding・実質変更なしでも `repeated_acceptance_findings` に到達しない。fingerprint から runtime-owned follow-up section を除外し、両モードの回帰テストを追加すること。
