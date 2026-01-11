## 1. 準備

- [ ] 1.1 `src/config/` ディレクトリを作成
- [ ] 1.2 基本的なモジュール構造を設定

## 2. デフォルト値の分離

- [ ] 2.1 `DEFAULT_*` 定数を `src/config/defaults.rs` に移動
- [ ] 2.2 グローバルパス定数（`PROJECT_CONFIG_FILE` など）も移動

## 3. JSONC パーサーの分離

- [ ] 3.1 `parse_jsonc` を `src/config/jsonc.rs` に移動
- [ ] 3.2 `strip_jsonc_features`, `remove_trailing_commas` を移動
- [ ] 3.3 JSONC 関連テストを移動

## 4. プレースホルダー展開の分離

- [ ] 4.1 `expand_change_id` を `src/config/expand.rs` に移動
- [ ] 4.2 `expand_prompt`, `expand_conflict_files` を移動
- [ ] 4.3 展開関連テストを移動

## 5. メインモジュールの整理

- [ ] 5.1 `OrchestratorConfig` を `src/config/mod.rs` に配置
- [ ] 5.2 `load`, `load_from_file` メソッドを配置
- [ ] 5.3 getter メソッドを配置
- [ ] 5.4 `src/config.rs` を削除

## 6. 依存関係の更新

- [ ] 6.1 `main.rs` のインポートを更新
- [ ] 6.2 他のモジュールのインポートを更新

## 7. テストと検証

- [ ] 7.1 テストを各モジュールに分散
- [ ] 7.2 `cargo test` で全テスト通過を確認
- [ ] 7.3 `cargo clippy` で警告がないことを確認
