## 1. 状態ロジックの追加

- [x] 1.1 AppState に全マーク/全アンマークのトグル関数を追加する（`src/tui/state.rs` に関数追加が確認できること）
- [x] 1.2 トグル対象の判定を単体トグルのガードと整合させる（並列モード未コミット除外などの条件が反映されていることをコードで確認）

## 2. キー入力と表示更新

- [x] 2.1 `x` キーで全件トグルを実行する（`src/tui/key_handlers.rs` に `KeyCode::Char('x')` の分岐が追加されていること）
- [x] 2.2 Changes パネルのキーヒントに `x: toggle all` を追加する（`src/tui/render.rs` の Changes パネルタイトルに反映されていること）

## 3. テストの追加

- [x] 3.1 全マーク/全アンマークの挙動を検証するユニットテストを追加する（`src/tui/state.rs` の tests に新規テストがあること）
- [x] 3.2 Running モードではヒントを表示しないことを確認するテストを追加する（`src/tui/render.rs` のテストに `x: toggle all` の有無を確認する検証があること）

## Acceptance #1 Failure Follow-up

- [x] `src/tui/render.rs` の `render_changes_list_running` で `app.mode == AppMode::Select` かつログ表示経路（`render()` が `app.logs.is_empty()` で分岐）でも `x: toggle all` を表示するよう修正し、Select モード要件（`openspec/changes/update-tui-toggle-all-marks/specs/tui-key-hints/spec.md` の Scenario: Select モードでヒントを表示する）を満たす。
- [x] 上記の回帰防止として、Select モードかつ `app.logs` が非空のときに `x: toggle all` が表示されるテストを `src/tui/render.rs` に追加する。
