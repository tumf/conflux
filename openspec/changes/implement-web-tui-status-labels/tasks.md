## 1. TUIのステータス語彙と表示を更新
- [ ] 1.1 QueueStatus語彙を新語彙へ変更する（完了条件: `src/tui/types.rs` のdisplayが `applying` などの語彙を返すことを確認する）
- [ ] 1.2 apply/acceptance/archive/resolve開始イベントで該当表示状態へ遷移する（完了条件: `src/tui/state/events/stages.rs` と `src/tui/state/events/processing.rs` の更新で `QueueStatus::Applying` 等が設定されることを確認する）
- [ ] 1.3 `status:iteration` 表示をTUIの行表示に反映する（完了条件: `src/tui/render.rs` のstatus表示が `applying:1` 形式になることを確認する）

## 2. Web UIのステータス表示/集計を更新
- [ ] 2.1 Web UIの表示語彙を新語彙へ更新する（完了条件: `web/app.js` のstatusIconsと表示ラベルが新語彙になることを確認する）
- [ ] 2.2 Web UIの集計指標を新語彙に合わせて更新する（完了条件: `web/app.js` の集計が applying/accepting/archiving/resolving を進行中として算出することを確認する）
- [ ] 2.3 Web UIの `status:iteration` 表示へ更新する（完了条件: `web/app.js` のカード表示が `applying:1` 形式になることを確認する）

## 3. 検証
- [ ] 3.1 `cargo test` を実行し、TUI関連テストが通ることを確認する（完了条件: 失敗がないこと）
