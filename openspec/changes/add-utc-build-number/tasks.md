## 1. 実装
- [ ] 1.1 ビルド番号の生成方法（UTC YYYYMMDDHHmmss）をバイナリに埋め込む
- [ ] 1.2 CLIの`--version`表示を`v<semver>(<build>)`形式に変更する
- [ ] 1.3 TUIヘッダのバージョン表示を`v<semver>(<build>)`形式に変更する
- [ ] 1.4 バージョン表示に関するテストを更新する

## 2. 検証
- [ ] 2.1 `cargo test` を実行して影響範囲のテストが通ること
- [ ] 2.2 `cargo fmt && cargo clippy -- -D warnings` を実行して整形とLintを通す
