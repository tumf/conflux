## Context

並列実行の orchestration loop (`src/parallel/orchestration.rs`) は、merge 進捗監視 (`src/merge_stall_monitor.rs`) と同じ `CancellationToken` を共有している。監視器が stall を検出すると `cancel_token.cancel()` を呼び、並列実行全体が即座に停止する。

これは「観測」と「制御」の責務が混在しており、監視ロジックの判断ミスや設計上の前提ずれがそのまま queue 実行の可用性に影響する。

## Goals / Non-Goals

- Goals:
  - 監視系を event emitter に変更し、実行制御から分離する
  - 監視系の不具合が並列実行を壊せない構造にする
  - 将来の monitor 追加が安全に行える拡張パターンを確立する

- Non-Goals:
  - merge stall 検出アルゴリズム自体の改善
  - supervisor/policy 層の UI (設定画面等) の実装
  - serial 実行系の全面的な再設計

## Decisions

- `MergeStallMonitor` は `CancellationToken` を受け取らず、`tokio::sync::mpsc::Sender<MergeStallEvent>` で stall イベントを送信する
  - Alternatives: (a) cancel_token の起点を monitor 開始時刻にする → 今回のバグは直るが責務混在は残る (b) monitor を完全削除する → 安全装置がなくなる
  - 選択理由: event channel 方式なら責務が明確に分離され、将来の policy 追加も容易

- デフォルト policy は warn-only とする
  - stall 検出時は `warn!` ログと `ParallelEvent::Warning` を送信
  - 実行停止はしない
  - 将来的に `.cflx.jsonc` の `merge_stall_detection.action` で `warn` / `soft_stop` / `hard_stop` を選べるようにする余地を残す

- monitor 自身の停止には専用の `CancellationToken` を使う
  - 並列実行終了時に monitor を cleanup するための token
  - 並列実行全体の cancel_token とは完全に独立

## Risks / Trade-offs

- stall 検出時に自動停止しなくなるため、本当に stuck した並列実行がリソースを消費し続ける可能性がある → 将来 policy 設定で `soft_stop` を選べるようにする
- event channel が受信されない場合 monitor 側で backpressure が発生する → bounded channel (capacity 16) で十分

## Open Questions

- なし（方針は会話で合意済み）
