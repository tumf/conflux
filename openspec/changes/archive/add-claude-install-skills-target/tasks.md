## Implementation Tasks

- [x] `src/cli.rs` の `install-skills` 引数に `--claude` を追加し、help text と examples を `.agents` / `.claude` の両方に対応させる（verification: unit - `src/cli.rs` の install-skills parse/help test が `--claude` と `--claude --global` を受理する）
- [x] `src/install_skills.rs` の install target 解決を target kind 対応に拡張し、`--claude` 時は skills path と lock file path を `.claude` 配下へ切り替える（verification: unit - install path resolution test が `project/global × agents/claude` の 4 ケースを検証する）
- [x] `tests/install_skills_test.rs` に Claude project/global install の filesystem regression を追加し、bundled skills と lock file が `.claude` 配下へ書かれることを確認する（verification: integration - `cargo test install_skills`）
- [x] `openspec/specs/cli/spec.md` と README 系 docs を更新し、`install-skills --claude` の user-facing behavior を明記する（verification: manual - `openspec/specs/cli/spec.md` と `README.ja.md` に `.claude` install examples が存在する）
- [x] 変更後の install-skills 系テストを実行し、既存 `.agents` 挙動が回帰していないことを確認する（verification: integration - `cargo test install_skills`、unit - `cargo test install_skills_cli --lib -- --nocapture` または相当する focused CLI test）

## Future Work

- 必要になった場合のみ、Claude 向け install target を他 README 言語版へ同期する別 proposal を切る
- Claude 向け install の lock file format 互換性を他ツールと調整する必要があれば別 proposal で扱う
