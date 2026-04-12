## Implementation Tasks

- [x] `src/tui/utils.rs` の change 解決ロジックを active / archive 両対応にする（verification: unit - `cargo test test_find_change_dir_ --lib -- --nocapture`）
- [x] archive entry の direct match と date-prefixed match を許容する探索を追加する（verification: unit - `src/tui/utils.rs` の archived fallback test が `openspec/changes/archive/<date>-<change_id>` を解決すること）
- [x] archived change fallback の回帰テストを追加し、既存 utility test を通す（verification: unit - `cargo test tui::utils --lib -- --nocapture`）

## Future Work

- 手元の TUI セッションで archived change を実際に選択して `e` を押す手動確認
- 必要なら act/exp 表示差分の別原因を追う追加 proposal を切り出す

## Acceptance #1 Failure Follow-up

- [x] `cargo test` を修正し、`server::api::projects::tests::test_projects_state_includes_sync_metadata_fields_after_monitor_refresh` が `POST /api/v1/projects` に対して `201 Created` を期待どおり返すようにする
- [x] archived change editor launch の完了条件に対する検証エビデンスを整合させる（`resolve_editor_target` の unit test を追加し、`proposal.md` 優先と directory fallback の両方を自動検証）

## Acceptance #2 Failure Follow-up

- [x] `cargo test` を修正し、`server::api::projects::tests::test_projects_state_includes_sync_metadata_fields_after_monitor_refresh` が `POST /api/v1/projects` で `201 Created` を返すようにする
- [x] `Implementation Tasks` の manual 完了記述と `Future Work` の手動確認記述を整合させる（manual を未完了へ戻すか、実施済み証跡を記録し、必要なら verification 種別を更新する）
