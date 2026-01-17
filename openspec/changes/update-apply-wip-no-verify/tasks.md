## 1. 仕様更新
- [ ] 1.1 apply プロンプトから `--no-verify` の一律禁止を外す要件を追加する（agent-prompts）
- [ ] 1.2 逐次 apply の WIP スナップショットで `git commit --no-verify --allow-empty` 相当を明記する（cli）
- [ ] 1.3 並列 apply の WIP スナップショットで `git commit --no-verify --allow-empty` 相当を明記する（parallel-execution）

## 2. 実装
- [ ] 2.1 `APPLY_SYSTEM_PROMPT` の `--no-verify` 禁止文言を削除する
- [ ] 2.2 Git バックエンドの WIP コミット作成に `--no-verify` を付与する

## 3. 検証
- [ ] 3.1 `cargo fmt` を実行する
- [ ] 3.2 `cargo clippy -- -D warnings` を実行する
- [ ] 3.3 `cargo test` を実行する
