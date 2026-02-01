## 1. 実装
- [x] 1.1 変更一覧（選択/実行モード）で各 change の最新ログを右側に単一行で表示する（相対時刻 + 短縮ヘッダ + メッセージ、幅に合わせて省略）。検証: `src/tui/render.rs` の描画テストを追加/更新して、プレビューが1行で表示されることを確認する
- [x] 1.2 ログヘッダ表示を `[operation:iteration]` / `[operation]` 形式に短縮し、change 名をヘッダから省略する。検証: ログヘッダ関連のテスト期待値が `[resolve:1]` になることを確認する
- [x] 1.3 既存テストを更新し、`cargo test` が成功することを確認する

## Acceptance #1 Failure Follow-up
- [x] `src/events.rs` の `LogEntry` に相対時刻計算用の実時間（例: `created_at`）を保持し、`src/tui/render.rs` の `render_changes_list_select` / `render_changes_list_running` で `just now` / `<n><unit> ago`（最大2単位、切り捨て）を描画時に算出する
- [x] `src/tui/render.rs` の `render_changes_list_select` / `render_changes_list_running` でプレビュー表示可能幅が10文字未満の場合はログプレビューを表示しない分岐を追加する

## Acceptance #2 Failure Follow-up
- [x] `src/tui/render.rs` の `render_changes_list_select` がプレビュー幅を固定値 `base_width = 55` で見積もっており、`WT`/`NEW`/`UNCOMMITED` バッジやタスク数の桁数、リスト枠幅を考慮しないため、実際の利用可能幅が10文字未満でもプレビューが表示されるケースがある。各行の実際の占有幅に基づいてプレビュー幅を算出する
- [x] `src/tui/render.rs` の `render_changes_list_running` がプレビュー幅を固定値 `base_width = 70` で見積もっており、`WT`/`NEW`/`UNCOMMITED` バッジやステータス/タスク/経過時間の表示幅、リスト枠幅を考慮しないため、実際の利用可能幅が10文字未満でもプレビューが表示されるケースがある。各行の実際の占有幅に基づいてプレビュー幅を算出する

## Acceptance #3 Failure Follow-up
- [x] Git 作業ツリーが dirty のため、`openspec/changes/update-tui-change-list-log-preview/tasks.md` と `src/tui/render.rs` の未コミット変更を解消する
- [x] `src/tui/render.rs` の `render_changes_list_select` / `render_changes_list_running` で占有幅の算出が実表示より短く（例: `format!("{} {} ", checkbox, cursor)` は6文字だが `checkbox_cursor_width=5`、ステータスや change_id の実長が固定幅を超える場合がある）ため、実表示幅に基づいて `available` を計算し、実際の利用可能幅が10文字未満ならプレビューを表示しないよう修正する
