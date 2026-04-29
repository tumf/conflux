## Implementation Tasks

- [x] 1. update archive readiness 契約の現行ズレ（`--strict` / evidence policy / commit-path blocker / archive未実施時の扱い）を spec と skill guidance で棚卸しし、canonical requirement に反映する (verification: manual - `cflx validate --strict` と `git diff -- openspec/specs/agent-prompts/spec.md openspec/specs/parallel-execution/spec.md skills/cflx-archive/SKILL.md src/openspec_cmd.rs` を実行して確認する)
- [x] 2. implement parallel archive command 実行で「archive CLI は未実施だが agent は exit 0 で自然文の blocker を返した」runtime failure ケースを、verification failure 一般論へ潰さず root-cause 付き archive failure として記録・表示する (verification: unit - `cargo test parallel::tests::executor -- --nocapture` または同等の追加テストで stdout/stderr tail 由来の blocker が最終 error に含まれることを確認する)
- [x] 3. implement serial / streaming archive command 実行でも、runtime failure の最終エラーに直前 attempt の validation failure または commit-path blocker 要約を保持する (verification: unit - `cargo test orchestration::archive::tests -- --nocapture` または同等の追加テストで final error が `not actually archived` だけで終わらないことを確認する)
- [x] 4. update archive prompt / skill guidance を更新し、archive agent が `cflx openspec archive <id> --yes` の前提条件 failure を検出した場合は archive 未実施の blocker として明示し、downstream が root cause を解釈できる出力方針を持つようにする (verification: manual - `skills/cflx-archive/SKILL.md`, `src/agent/prompt.rs`, `src/agent/runner.rs` の archive-related guidance/paths を確認し、archive-readiness blocker の扱いが追加されていることをレビューする)
- [x] 5. verify proposal/spec/skill/実装を通しで、archive readiness と failure-reporting 契約が同じ change failure 例で一貫することを確認する (verification: integration - `cflx openspec validate align-archive-readiness-failure-reporting --strict --evidence warn`、`cargo test`、`cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- `apply` / `acceptance` / `resolve` でも同様の「exit code success だが machine-readable failure を返す」契約不整合が見つかった場合は、別 change で共通 protocol 化する

## Acceptance #1 Failure Follow-up
- [x] `src/orchestration/archive.rs` の serial-path regression test を default 経路で実行しつつ 1 秒未満へ最適化しました。具体的には `test_archive_change_reports_runtime_blocker_when_archive_not_started` の `OrchestratorConfig` に `command_queue_stagger_delay_ms: Some(0)` を設定し、デフォルト検証 `agent-exec run -- cargo test orchestration::archive::tests -- --nocapture`（job `b5ddab1963b2654247e315bea3f9ca66`）で 4 件実行・対象テスト成功、同 job の stdout で `finished in 0.07s` を確認しました。これにより『exit 0 だが archive-start blocker を返す serial-path case の回帰検証』は default スイートで継続的に担保されています。
## Acceptance #2 Failure Follow-up
- [x] `src/orchestration/archive.rs` の serial/non-streaming 実装は blocker summary を最終エラーへ伝播するよう修正されていますが、前回指摘で要求された serial-path regression test が未追加です。`src/orchestration/archive.rs:696-866` の tests には blocker summary を検証する新規テストがなく、`cargo test orchestration::archive::tests` でも既存 3 テストしか実行されていません（job `8ce3c19d37647d46f8087e781cfbf836`）。`tasks.md:5` と acceptance follow-up の要件どおり、exit 0 だが archive-start blocker を返したケースで final error に blocker summary が含まれることを検証する serial-path regression test を追加してください。

## Acceptance #3 Failure Follow-up
- [x] 追加された serial-path regression test `src/orchestration/archive.rs:749-836` は前回指摘自体は解消していますが、`cargo test orchestration::archive::tests -- --nocapture` で 4.09s かかっており（job `25abae6e6190f87ff1d2a7ad0261c90c`, `stdout.log:8`）、このリポジトリの『1秒超のテストは最適化するか heavy 扱いにする』規約に反しています。デフォルトスイートに残すなら 1 秒未満へ短縮し、難しければ heavy テストとして分離してください。

## Acceptance #4 Failure Follow-up
- [x] 通常コミット経路の前提確認として `agent-exec run -- cargo test orchestration::archive::tests` を再実行すると、`src/orchestration/archive.rs:699-704` に unused import 6 件の warning が残っています（job `579a27342bdc20f7389a999077dbe14f`, stderr）。`tasks.md:7` では `cargo clippy --all-targets --all-features -- -D warnings` を検証要件としており、archive 最終コミット前の実運用経路で warning-free を満たせる状態ではありません。heavy test 化で不要になった import を削除し、warnings を解消してください。（実施: testモジュールの import を heavy test 関数内へ移動し、`agent-exec run -- cargo test orchestration::archive::tests` job `5ff35b01d059755893a53189d5799b57` で warning なしを確認）
