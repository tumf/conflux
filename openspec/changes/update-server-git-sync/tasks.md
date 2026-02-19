## 1. API 追加と互換経路

- [x] 1.1 `POST /api/v1/projects/{id}/git/sync` を追加する（`src/server/api.rs` にルートとハンドラが追加されていることを確認）
- [x] 1.2 既存 `git/pull` / `git/push` は `sync` に委譲する（`src/server/api.rs` の実装が共通化されていることを確認）

## 2. resolve 必須化とレスポンス

- [x] 2.1 `sync` は resolve_command を必ず実行する（未設定時は明示エラーになることを確認）
- [x] 2.2 `sync` のレスポンスに pull/push の結果を含める（JSON に pull/push セクションが含まれることを確認）

## 3. テスト

- [x] 3.1 resolve_command 未設定の `sync` が失敗するテストを追加する（エラーメッセージが返ることを確認）
- [x] 3.2 `sync` が pull/push の結果を返すテストを追加する（両方の結果が JSON に含まれることを確認）

## Acceptance #1 Failure Follow-up

- [x] `src/server/api.rs` の `git_pull` / `git_push` を `git_sync` へ委譲し、重複実装を解消する（`git_sync` のみが pull/push 本体ロジックを持つ状態にする）。
- [x] `src/server/api.rs` の `git_sync` で non-fast-forward の有無に関係なく `resolve_command` を必ず 1 回実行し、失敗時は同期失敗を返すように修正する（以前は `merge-base --is-ancestor` 失敗時のみ実行されていた）。
- [x] `src/server/api.rs` の `test_git_sync_response_contains_pull_and_push_sections_on_error` を、成功レスポンスで `pull` と `push` セクションの存在を検証するテスト（`test_git_sync_success_response_contains_pull_and_push_sections`）に置き換える（ローカル bare remote を使ったフィクスチャで外部依存なしに検証する）。
