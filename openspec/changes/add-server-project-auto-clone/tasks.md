## 1. API 追加時の自動クローン

- [ ] 1.1 `POST /api/v1/projects` でブランチ検証と bare clone を行う（検証: `src/server/api.rs` の add_project が clone 処理を呼び出す）
- [ ] 1.2 `data_dir/worktrees/<project_id>/<branch>` に作業ツリーを作成する（検証: add_project 経路で worktree 作成処理が呼ばれる）

## 2. 排他・ロールバック

- [ ] 2.1 add_project でも global semaphore と project lock を適用する（検証: add_project で acquire/lock を実施）
- [ ] 2.2 clone/worktree 失敗時に registry へ残さない（検証: 失敗時の削除処理が実装されている）

## 3. テスト

- [ ] 3.1 ローカル `file://` の一時リポジトリで add_project の自動クローン成功テストを追加する（検証: `src/server/api.rs` のテストが `cargo test` で通る）
- [ ] 3.2 ブランチ未存在や clone 失敗時に 4xx を返し registry が更新されないテストを追加する（検証: 同ファイルのテストが `cargo test` で通る）
