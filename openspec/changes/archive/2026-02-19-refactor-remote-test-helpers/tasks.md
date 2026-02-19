## 1. Characterization
- [x] 1.1 リモート関連テストのキャラクタリゼーション（検証: `cargo test remote`）

## 2. Refactor
- [x] 2.1 WS モックサーバー生成の共通ヘルパーを抽出（検証: `cargo test remote`）
- [x] 2.2 HTTP モックサーバー生成の共通ヘルパーを抽出（検証: `cargo test remote`）
- [x] 2.3 JSON フィクスチャ/検証補助の共有化（検証: `cargo test remote`）

## Acceptance #1 Failure Follow-up
- [x] `src/remote/ws.rs` の `test_bearer_token_sent_in_ws_upgrade`（約 L235-L275）で `TcpListener::bind` を直接使っており、`openspec/changes/refactor-remote-test-helpers/specs/code-maintenance/spec.md` の MUST（WS/HTTP モックサーバー生成を共通ヘルパー経由にする）を満たしていません。ヘッダー検証を維持したまま `src/remote/test_helpers.rs` の共通ヘルパー（必要なら新規 helper）経由へ置き換えてください。
