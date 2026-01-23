## 1. 状態モデルの拡張
- [ ] 1.1 `QueueStatus`に`ResolveWait`を追加し、表示文字列/色を定義する（`src/tui/types.rs`）
  - 完了条件: `QueueStatus::ResolveWait`がenumと`display()`/`color()`に追加されている

## 2. Resolve待ち状態の遷移
- [ ] 2.1 resolveシリアライズで待機が発生したchangeを`ResolveWait`として通知する（`src/parallel/mod.rs` または関連イベント処理）
  - 完了条件: resolve待機中のchangeに対しTUIへ`ResolveWait`状態が反映される経路がある

## 3. TUI更新と操作制御
- [ ] 3.1 自動更新で`ResolveWait`を`NotQueued`へ戻さないようにする（`src/tui/state/events/helpers.rs`）
  - 完了条件: `ResolveWait`が保持される分岐が追加されている
- [ ] 3.2 `ResolveWait`中はSpace/@でキュー状態を変更できないようにする（`src/tui/state/mod.rs`）
  - 完了条件: `toggle_selection`/`toggle_approval`で`ResolveWait`が変更対象外になっている
- [ ] 3.3 Changesリストで`ResolveWait`のステータス表示を確認できるようにする（`src/tui/render.rs`）
  - 完了条件: ステータス表示に`resolve wait`が反映される

## 4. テスト
- [ ] 4.1 `ResolveWait`の自動更新保持と操作ブロックのテストを追加する（`src/tui/state/mod.rs` または該当テストモジュール）
  - 完了条件: `cargo test tui::state` で該当テストが通る
