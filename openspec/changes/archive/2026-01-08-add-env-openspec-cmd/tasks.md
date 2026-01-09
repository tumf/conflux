# Tasks: 環境変数で openspec コマンドを設定可能にする

## 1. Implementation

- [x] 1.1 `src/cli.rs` の `openspec_cmd` フィールドに `env` 属性を追加
- [x] 1.2 環境変数の優先順位が正しく動作することを確認（CLI > env > default）

## 2. Testing

- [x] 2.1 環境変数が設定されていない場合、デフォルト値が使用されることを確認
- [x] 2.2 環境変数が設定されている場合、その値が使用されることを確認
- [x] 2.3 CLI 引数が環境変数より優先されることを確認

## 3. Documentation

- [x] 3.1 README に環境変数オプションを記載（必要に応じて）
