## 1. 変更方針の確認
- [x] 1.1 `command-queue` の Streaming リトライ要件と現状実装の差分を整理する
- [x] 1.2 streaming 実行経路（apply/archive/resolve）の適用ポイントを特定する

## 2. 実装
- [x] 2.1 streaming 実行で retry 判定を行うフローに変更する
- [x] 2.2 リトライ通知を出力チャネルへ送信する
- [x] 2.3 apply/archive/resolve の各経路で同一の挙動になることを確認する

## 3. 検証
- [x] 3.1 streaming 実行のリトライに関する単体テストを更新または追加する
- [x] 3.2 `cargo test` で関連テストを実行する
