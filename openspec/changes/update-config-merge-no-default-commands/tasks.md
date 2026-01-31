## 1. Implementation
- [ ] 1.1 設定読み込みをマージ型に変更する（platform < XDG < project < custom の順で `Some` のみ上書きする）
  - 完了条件: `src/config/mod.rs` の `load()` が複数ファイルを読み込み、項目ごとのマージを行う
  - 検証: 追加したユニットテストで、project が部分設定でも global の値が残ることを確認する
- [ ] 1.2 hooks のディープマージを実装する
  - 完了条件: `HooksConfig` の各フィールド（`on_start`, `pre_apply` 等）が個別にマージされる
  - 検証: global と project で異なる hook を設定し、両方が有効になることを確認するテストを追加する
- [ ] 1.3 コマンド設定の必須化とエラーメッセージを追加する（apply/archive/analyze/acceptance/resolve）
  - 完了条件: これらのコマンドが未設定のとき設定ロード完了時にバリデーションが失敗し、欠落キーを含むエラーが返る
  - 検証: `src/config/mod.rs` の新規テストで、欠落時にエラーとなることを確認する
- [ ] 1.4 既定コマンドのフォールバックを廃止する（DEFAULT_*_COMMAND を使用しない）
  - 完了条件: `get_*_command()` が既定値に落ちない実装になり、未設定時は 1.3 のエラー経由で停止する
  - 検証: 既存テストの更新と新規テストで、未設定時に既定値が使われないことを確認する
- [ ] 1.5 既存テストを更新する
  - 完了条件: `test_load_returns_default_when_no_config_exists` 等の DEFAULT_*_COMMAND を期待するテストを、マージ型・必須化に合わせて修正する
  - 検証: `cargo test` が成功する

## 2. Validation
- [ ] 2.1 `cargo fmt --check` と `cargo clippy -- -D warnings` が成功することを確認する
- [ ] 2.2 `cargo test` を実行して設定ロード関連のテストが通ることを確認する
