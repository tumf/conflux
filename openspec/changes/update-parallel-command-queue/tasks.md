## 1. CommandQueue への切り替え
- [x] 1.1 parallel executor 内に shared stagger state を作成して使い回す
- [x] 1.2 apply 実行を CommandQueue 経由に切り替える（stagger + retry + streaming）
- [x] 1.3 archive 実行を CommandQueue 経由に切り替える（stagger + retry + streaming）

## 2. 出力/リトライ通知の維持
- [x] 2.1 リトライ通知が TUI/CLI のログに表示されることを確認する
- [x] 2.2 既存の ParallelEvent の出力順序が変わらないことを確認する

## 3. 検証
- [x] 3.1 並列実行で stagger が適用されることを確認する
- [x] 3.2 `cargo test` を実行して回帰がないことを確認する
