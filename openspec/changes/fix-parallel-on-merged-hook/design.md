## Context
パラレルモードの自動マージ成功時に`on_merged`フックが実行されない経路があり、既存仕様の期待と実装がずれている。

## Goals / Non-Goals
- Goals: パラレルモードの全マージ成功経路で`on_merged`を必ず実行する。
- Non-Goals: フック実行順序や新しいフック種別の追加。

## Decisions
- Decision: `MergeAttempt::Merged`の成功分岐に`on_merged`呼び出しを追加する。
- Alternatives considered: `MergeCompleted`イベント側で一括実行する案は、イベント発行経路の差分が多くなるため採用しない。

## Risks / Trade-offs
- 既存の並列処理に追加のフック実行が入るため、フックの失敗ログが増える可能性がある。

## Migration Plan
既存の設定はそのまま利用できる。実装更新後にパラレル実行でのマージ完了時に`on_merged`が発火することを確認する。

## Open Questions
- なし
