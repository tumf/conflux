## 1. 要件反映
- [ ] 1.1 `parallel-execution` の仕様差分を作成する（WIP 反復スナップショット、最終 squash、イテレーション番号）

## 2. 実装
- [ ] 2.1 apply ループの各イテレーション終了時にスナップショット作成を行う（進捗有無に関わらず）
- [ ] 2.2 WIP コミットメッセージに `apply#{iteration}` を付与する
- [ ] 2.3 JJ バックエンドでスナップショットと squash を実行する
- [ ] 2.4 Git バックエンドでスナップショットと squash を実行する
- [ ] 2.5 失敗時は WIP を保持し、最終 squash を行わない

## 3. ログ・イベント
- [ ] 3.1 apply 進捗ログにイテレーション番号を含める
- [ ] 3.2 進捗コミット作成時のログを整備する

## 4. 検証
- [ ] 4.1 `cargo test` を実行する
- [ ] 4.2 既存の並列実行系テスト（`tests/e2e_tests.rs`）の結果を確認する
- [ ] 4.3 `RUST_LOG=debug cargo run -- run --dry-run` で WIP と Apply のログを確認する
- [ ] 4.4 `RUST_LOG=info cargo run -- run --dry-run` でユーザー向けログのみ表示されることを確認する
