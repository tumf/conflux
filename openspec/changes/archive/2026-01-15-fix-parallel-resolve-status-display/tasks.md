## 1. 実装
- [x] 1.1 `src/parallel/mod.rs` の `merge_and_resolve` で `resolve_conflicts_with_retry` 呼び出し前に対象 change_id の `ResolveStarted` を送る
- [x] 1.2 `src/parallel/mod.rs` の `resolve_merge_for_change` で解決開始時に `ResolveStarted` を送る（既存の手動resolve用）
- [x] 1.3 `src/parallel/conflict.rs` の `resolve_merges_with_retry` で各 change_id に対して `ResolveStarted` を送る
- [x] 1.4 解決完了/失敗時に適切に `ResolveCompleted` / `ResolveFailed` が送信されることを確認

## 2. テスト
- [x] 2.1 parallel実行で衝突解決開始時に `ResolveStarted` イベントが送信されることをテスト
- [x] 2.2 TUI側で `ResolveStarted` を受けて `QueueStatus::Resolving` に遷移することをテスト（既存テスト確認）
- [x] 2.3 複数 change の順次マージで各 change に対して正しくイベントが送信されることをテスト

## 3. 検証
- [x] 3.1 `npx @fission-ai/openspec@latest validate fix-parallel-resolve-status-display --strict`
- [x] 3.2 `cargo test` で既存テストが全て通ることを確認
- [x] 3.3 TUIでparallel実行時、衝突解決中のchangeが「resolving」ステータスで表示されることを確認
