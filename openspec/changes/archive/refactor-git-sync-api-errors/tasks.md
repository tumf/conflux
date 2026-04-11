# タスク: APIレイヤ (git_sync) のエラーハンドリング改善

## Implementation Tasks
- [x] **[確認]** 既存の `git_sync` API のテストを実行し、すべてのテストがパスすることを確認する (Characterization tests)。
- [x] **[リファクタリング]** `src/server/api/git_sync.rs` 内の `unwrap/expect` を特定し、適切なエラー型を返すようにハンドラーを修正する。エラーの伝播には `?` 演算子を使用し、APIレスポンスとして適切にマッピングする。
- [x] **[検証]** 既存の正常系テストがパスすること、および、リファクタリング前はパニックを起こしていたかもしれない異常系リクエストに対して、適切なHTTPエラーレスポンス (4xx, 5xx) が返されることを確認する。

## Verification Plan
unit: `build_resolve_command_argv`, `run_resolve_command` と、pure helper の unit test で主要分岐を確認する。
integration: API ルーター経由で `/api/v1/projects/:id/git/sync` の正常系・異常系レスポンスを確認する。

## Acceptance #1 Failure Follow-up
- [x] `git_sync` の既存ルーターテストを integration に再分類し、Verification Plan の unit から外す。
- [x] `git_sync` の主要分岐で unit 検証が必要なら、実 filesystem / git process に依存しない純粋ロジックを抽出して unit test を追加する。
