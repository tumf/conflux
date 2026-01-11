# Change: アーカイブ失敗時にステータスがArchivingのまま残るバグを修正

## Why

並列実行モードでアーカイブに失敗した場合、`ParallelEventBridge` が `ArchiveFailed` イベントを `ProcessingError` に変換しないため、TUI 上でステータスが `Archiving` のまま残り、エラー状態に遷移しない。

## What Changes

- `parallel_event_bridge.rs` の `ArchiveFailed` イベント処理を修正
  - `ProcessingError` イベントを追加で生成するように変更
  - `ApplyFailed` と同様のパターンに統一

## Impact

- 影響を受ける spec: `parallel-execution`
- 影響を受けるコード: `src/tui/parallel_event_bridge.rs`
- 影響範囲: 並列実行モードのみ（シーケンシャルモードには影響なし）
