## Implementation Tasks

- [x] `src/tui/utils.rs` の change 解決ロジックを active / archive 両対応にする（verification: unit - `cargo test test_find_change_dir_ --lib -- --nocapture`）
- [x] archive entry の direct match と date-prefixed match を許容する探索を追加する（verification: unit - `src/tui/utils.rs` の archived fallback test が `openspec/changes/archive/<date>-<change_id>` を解決すること）
- [x] `launch_editor_for_change()` が解決済み path から `proposal.md` または directory fallback を開くことを維持する（verification: manual - TUI Changes view で archived change を選び `e` で editor launch が失敗しないことを確認）
- [x] archived change fallback の回帰テストを追加し、既存 utility test を通す（verification: unit - `cargo test tui::utils --lib -- --nocapture`）

## Future Work

- 手元の TUI セッションで archived change を実際に選択して `e` を押す手動確認
- 必要なら act/exp 表示差分の別原因を追う追加 proposal を切り出す

## Acceptance #1 Failure Follow-up

- [x] `cargo test` を修正し、`server::api::projects::tests::test_projects_state_includes_sync_metadata_fields_after_monitor_refresh` が `POST /api/v1/projects` に対して `201 Created` を期待どおり返すようにする
- [x] archived change editor launch の完了条件に対する検証エビデンスを整合させる（`resolve_editor_target` の unit test を追加し、`proposal.md` 優先と directory fallback の両方を自動検証）
