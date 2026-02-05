## 1. 実装
- [ ] 1.1 `AppState` に resolve 待ち行列（FIFO + 重複防止）を追加する
  - 確認: `src/tui/state.rs` に新しいキュー構造と操作関数が追加されている
- [ ] 1.2 `M` 押下時の resolve 分岐を更新し、resolve 実行中は `ResolveWait` へ遷移してキューへ追加する
  - 確認: `src/tui/state.rs` の `resolve_merge` 相当処理が resolve 実行中でもコマンド待ちを作れる
- [ ] 1.3 resolve 完了イベントで次の resolve を自動開始するフローを追加する
  - 確認: `src/tui/runner.rs` のイベント処理で次の `ResolveMerge` が送信される
- [ ] 1.4 resolve 失敗時は自動開始しない挙動を追加し、キューを保持する
  - 確認: `ResolveFailed` 後に次の resolve が開始されないことが分かる処理がある
- [ ] 1.5 `M` キーヒントを resolve 実行中/非実行中で出し分ける
  - 確認: `src/tui/render.rs` に `M: resolve` / `M: queue resolve` の分岐がある

## 2. テスト
- [ ] 2.1 `M` キーヒントの表示条件テストを更新する
  - 確認: `src/tui/render.rs` の既存テストが新仕様を検証している
- [ ] 2.2 resolve 待ち行列のシリアライズ動作を検証するテストを追加する
  - 確認: resolve 完了後に次の change が開始されるテストがある

## 3. 検証
- [ ] 3.1 `cargo test` を実行する
  - 確認: 失敗がないこと
