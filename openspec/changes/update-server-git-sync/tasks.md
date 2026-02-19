## 1. API 追加と互換経路

- [x] 1.1 `POST /api/v1/projects/{id}/git/sync` を追加する（`src/server/api.rs` にルートとハンドラが追加されていることを確認）
- [x] 1.2 既存 `git/pull` / `git/push` は `sync` に委譲する（`src/server/api.rs` の実装が共通化されていることを確認）

## 2. resolve 必須化とレスポンス

- [x] 2.1 `sync` は resolve_command を必ず実行する（未設定時は明示エラーになることを確認）
- [x] 2.2 `sync` のレスポンスに pull/push の結果を含める（JSON に pull/push セクションが含まれることを確認）

## 3. テスト

- [x] 3.1 resolve_command 未設定の `sync` が失敗するテストを追加する（エラーメッセージが返ることを確認）
- [x] 3.2 `sync` が pull/push の結果を返すテストを追加する（両方の結果が JSON に含まれることを確認）
