## 1. 実装
- [x] 1.1 変更一覧（選択/実行モード）で各 change の最新ログを右側に単一行で表示する（相対時刻 + 短縮ヘッダ + メッセージ、幅に合わせて省略）。検証: `src/tui/render.rs` の描画テストを追加/更新して、プレビューが1行で表示されることを確認する
- [x] 1.2 ログヘッダ表示を `[operation:iteration]` / `[operation]` 形式に短縮し、change 名をヘッダから省略する。検証: ログヘッダ関連のテスト期待値が `[resolve:1]` になることを確認する
- [x] 1.3 既存テストを更新し、`cargo test` が成功することを確認する

## Acceptance #1 Failure Follow-up
- [x] `src/events.rs` の `LogEntry` に相対時刻計算用の実時間（例: `created_at`）を保持し、`src/tui/render.rs` の `render_changes_list_select` / `render_changes_list_running` で `just now` / `<n><unit> ago`（最大2単位、切り捨て）を描画時に算出する
- [x] `src/tui/render.rs` の `render_changes_list_select` / `render_changes_list_running` でプレビュー表示可能幅が10文字未満の場合はログプレビューを表示しない分岐を追加する
