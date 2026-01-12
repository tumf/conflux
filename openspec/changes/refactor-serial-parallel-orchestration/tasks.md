## 1. 調査と設計
- [ ] 1.1 serial/parallel の重複箇所を棚卸し（apply/archiving/進捗/フック）
- [ ] 1.2 共有 API の候補と責務境界を決定

## 2. 共通ロジック化
- [ ] 2.1 共有実行フロー（apply/archiving/検証）を `src/execution/` または専用 helper に集約
- [ ] 2.2 serial/parallel の呼び出し側を共通 API に置き換え
- [ ] 2.3 モード固有のイベント/出力処理を明確化

## 3. 検証
- [ ] 3.1 `cargo fmt` を実行
- [ ] 3.2 `cargo clippy -- -D warnings` を実行
- [ ] 3.3 `cargo test` を実行
