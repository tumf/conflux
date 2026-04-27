## Implementation Tasks

- [x] `src/openspec_cmd.rs` の `OpenSpecManager::list_changes()` から `openspec/changes/archive/` の列挙を除外し、`cflx openspec list` が active change のみを返すようにする(verification: unit + cli - archived entry を含む fixture/repo test で list 出力に archived id が含まれないこと、および `cflx openspec list` 実行で archived change が表示されないことを確認)
- [x] `src/openspec_cmd.rs` の list 表示分岐を active-only contract に合わせて整理し、archived 表示前提の文言や status rendering を更新する（verification: unit - focused openspec command tests が active list output のみを期待すること）
- [x] `show` の archived resolution が維持される focused regression test を追加または更新する（verification: unit - `cflx openspec show <archived-change>` 相当の native handler test が archive entry を解決すること）
- [x] strict proposal validation と focused Rust tests を実行し、list/show の contract split を回帰保護する（verification: unit - `cflx openspec validate hide-archived-openspec-list --strict` と `cargo test openspec_list_show_tests -- --nocapture` を実行）

## Future Work

- archived change を一覧したい運用向けに別コマンドまたは flag を追加するかの検討
- TUI / Web UI 側でも archived/noise separation を揃えるべきかの別 proposal 検討

## Acceptance #1 Failure Follow-up
- [x] Commit-path blocker resolved: removed unused `ChangeInfo.archived` field in `src/openspec_cmd.rs` and verified `cargo clippy --locked --all-targets --all-features -- -D warnings` passes (job: /Users/tumf/.local/share/agent-exec/jobs/88d5c92b65ca80c0bd739fd008863218).
- [x] Re-verified implementation contract after blocker fix: `list_changes()` still excludes archived entries, focused tests and strict validation pass (`cargo test openspec_list_show_tests -- --nocapture`, `cflx openspec validate hide-archived-openspec-list --strict`).
