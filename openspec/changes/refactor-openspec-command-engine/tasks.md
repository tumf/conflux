## Implementation Tasks

- [x] 1. Characterization: `cflx openspec` の主要 entrypoint contract を固定する。verification: unit - `cargo test openspec_cmd` で list/show/validate/archive 関連の既存テストが成功し、必要なら CLI rendering/exit contract のテストを追加する。completion: list/show/validate/archive の代表結果が refactor 前にテストで確認できる。

- [x] 2. Characterization: spec promotion engine の挙動を固定する。verification: unit - `cargo test merge_spec_delta`、`cargo test delta_to_canonical`、`cargo test simulate_promotion` 相当のテストが成功する。completion: ADDED/MODIFIED/REMOVED、no-op rejection、missing target error がテストで固定されている。

- [x] 3. validation 責務を内部モジュールまたは helper 群へ分離する。verification: unit - `cargo test validate_tasks` と `cargo test openspec_cmd` が成功し、strict validation のエラー・警告挙動が維持されている。completion: proposal/tasks/spec delta/task evidence validation が archive 実行や rendering から分離して読める構造になっている。

- [x] 4. archive/promotion update 責務を内部モジュールまたは helper 群へ分離する。verification: unit - `cargo test archive` と promotion simulation 関連テストが成功し、archive 前 validation → simulation → move → spec update の順序が維持されている。completion: archive 実行と canonical spec 更新の境界が明確になり、失敗時の戻り値 contract が変わっていない。

- [x] 5. rendering/dependency status 補助処理を entrypoint から分離する。verification: unit - `cargo test render_show`、`cargo test dependency` または該当既存テストが成功する。completion: CLI 表示整形と workspace dependency status 計算が validation/archive ロジックと混在していない。

- [x] 6. 最終回帰確認を実行する。verification: integration - `cargo fmt --check`、`cargo test`、`cflx openspec list --specs` が成功する。completion: 既定テストスイートと OpenSpec list の実行が成功し、CLI contract の意図しない変更がない。

## Future Work

より大きな `openspec_cmd` の公開 API 再設計は別提案で扱う。

## Final Validation

実装後の OpenSpec 最終確認は `cflx openspec validate refactor-openspec-command-engine --strict` を使用する。

## Acceptance #1 Failure Follow-up
- [x] Archive commit path pre-commit blocker is fixed. The unused promotion public re-export was removed from `src/openspec_cmd.rs`, promotion tests now import the helpers directly from `crate::openspec_cmd::promotion`, and `agent-exec run -- cargo clippy --locked --all-targets --all-features -- -D warnings` completed successfully in job `edfdb113aea17c828e9a9708365b9cda`.
- [x] The implementation otherwise has relevant evidence: `cflx openspec validate refactor-openspec-command-engine --strict` passed, `cargo fmt --check` passed, `cargo test openspec_cmd` passed 66 tests, `cflx openspec list --specs` succeeded, and a rerun of the full `cargo test` passed after an initial flaky/concurrent failure. The remaining clippy blocker from Acceptance #1 has now been resolved.
