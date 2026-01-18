## 1. 実装
- [ ] 1.1 CLIサブコマンドの設計と引数仕様を確定する
- [ ] 1.2 spec deltaの解析ロジックを実装する（ADDED/MODIFIED/REMOVED/RENAMEDの抽出）
- [ ] 1.3 衝突検出ルールを実装する（同一Requirementの内容差分・削除との競合・rename競合）
- [ ] 1.4 CLI出力の整形（人間向け/JSON）と終了コードの実装

## 2. テスト
- [ ] 2.1 解析ロジックのユニットテストを追加する
- [ ] 2.2 衝突種別ごとの検出テストを追加する
- [ ] 2.3 CLIの統合テストを追加する

## 3. 検証
- [ ] 3.1 cargo fmt を実行する
- [ ] 3.2 cargo clippy -- -D warnings を実行する
- [ ] 3.3 cargo test を実行する
