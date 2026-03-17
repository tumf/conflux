## Context
server モードの WebSocket は FullState の定期スナップショットのみを配信しており、runner の stdout/stderr や実行中ログはリモート TUI に届かない。結果として「F5 で開始はされるがログが流れない」状態になる。

## Goals / Non-Goals
- Goals:
  - server 実行ログを WebSocket で配信し、remote TUI に表示する
  - TUI のログ表示語彙・ヘッダ規則に合わせて表示する
- Non-Goals:
  - Web ダッシュボード UI の新規表示機能追加
  - ログの永続化/検索機能の追加

## Decisions
- Decision: server が `RemoteStateUpdate::Log` を WebSocket で配信する
  - 理由: 既存の TUI イベントモデルに最小の追加で統合できる
- Decision: server runner の stdout/stderr を行単位で LogEntry 化する
  - 理由: 既存のログパネル表示単位と整合する

## Risks / Trade-offs
- ログ量が多いプロジェクトでは WS 帯域が増える
  - Mitigation: 直近 N 行の保持/サーバ側での簡易トリムを行う

## Migration Plan
- 追加の互換性破壊はなし（Log イベントは追加配信のみ）

## Open Questions
- なし
