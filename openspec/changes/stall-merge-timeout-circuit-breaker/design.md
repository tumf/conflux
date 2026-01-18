## Context
- ベースブランチへの merge 進捗が止まると、実行中の apply/archive が完了しても全体が停止していることに気づきにくい。
- serial/parallel の両モードで共通に、merge 進捗の停滞を検知して即時停止する必要がある。

## Goals / Non-Goals
- Goals:
  - 30分間 merge 進捗がない場合にオーケストレーションを stall と判定し即時停止する。
  - serial/parallel 両方に適用する。
  - 停止理由をイベント/ログに伝播する。
  - 閾値と監視間隔を設定で上書き可能にする。
- Non-Goals:
  - merge 停滞の原因解析や自動復旧は行わない。
  - 個別 change 単位の停止は行わない（全停止のみ）。

## Decisions
- 監視は Tokio タスクで実装し、`tokio::time::interval` により定期チェックする。
- merge 進捗はベースブランチの `Merge change: <change_id>` コミットが最後に現れた時刻で判定する。
- stall 検知時は `CancellationToken` を即時発火し、既存の停止経路に合流させる。

## Risks / Trade-offs
- 監視間隔が短すぎると git log の実行頻度が高くなるため、デフォルトは現状の run loop 負荷を増やさない設定値にする。
- stall 検知は merge 進捗のみを基準とするため、長時間の apply が発生する場合は誤検知の可能性がある。

## Migration Plan
- 設定項目を追加し、未設定時は既定値で動作するようにする。
- 既存の stall/circuit-breaker ロジックとは独立に動作させる。

## Open Questions
- なし（要求定義済み）
