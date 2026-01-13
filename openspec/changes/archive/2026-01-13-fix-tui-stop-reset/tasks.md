## 1. Implementation
- [x] 1.1 実行開始時に停止/キャンセル状態をリセットする処理を追加する
- [x] 1.2 Stopped モードの再開フローで parallel 切替後も正常開始することを確認する
- [x] 1.3 停止状態リセットのログ/状態遷移を調整する

## 2. Tests
- [x] 2.1 Stopped からの再開で停止フラグが残らないことをテストする
- [x] 2.2 serial 停止 → parallel 切替 → 再開のシナリオを追加する

## 3. Validation
- [x] 3.1 `cargo test`（関連テスト）を実行する
