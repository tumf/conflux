## Implementation Tasks

- [ ] 1. server project 追加フローで `repo_root/.wt/setup` を実行する（verification: `src/server/api.rs` の `POST /api/v1/projects` 成功経路に setup 実行呼び出しが追加され、`~/.wt/setup` ではなく repo-root を対象にしていることを確認する）

- [ ] 2. setup 実行失敗時の rollback を server 追加フローに統合する（verification: setup 失敗時に API が非成功を返し、registry へ当該 project が残らないことをテストで確認する）

- [ ] 3. server mode で setup 実行の有無を検証するテストを追加する（verification: `.wt/setup` あり/なし/失敗 の各ケースを `src/server/api.rs` テストで確認する）

- [ ] 4. spec を更新して server mode の `.wt/setup` 振る舞いを明確化する（verification: `openspec/changes/fix-server-worktree-setup-parity/specs/server-mode/spec.md` に MODIFIED Requirement と Scenario が追加される）

## Future Work

- server mode と通常 worktree 作成経路の完全共通化（重複処理を helper に集約）
