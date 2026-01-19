## Context
Web UIは現在、queue_statusが未設定の場合にlegacy status（pending/in_progress/complete）へフォールバックし、統計も同じ基準で算出している。この挙動はTUIのQueueStatusと一致せず、監視の整合性を損なう。

## Goals / Non-Goals
- Goals: Web UIのステータス表示・集計をTUIのQueueStatus語彙に統一する。
- Non-Goals: TUI自体のステータス設計変更、バックエンドのイベント設計変更。

## Decisions
- Decision: Web UIの表示/集計はQueueStatusのみを使用し、legacy statusのフォールバックを廃止する。
- Decision: Acceptingの表示を"accepting"として明示し、専用のバッジ/アイコンを追加する。

## Risks / Trade-offs
- legacy statusのみを前提にした外部実装がある場合、表示が変わる可能性がある。

## Migration Plan
- Web UIの表示・集計ロジックを更新し、QueueStatusのみに統一する。
- UIのステータスバッジ/アイコン定義を拡充する。

## Open Questions
- なし
