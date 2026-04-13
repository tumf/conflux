## Context
既存の single-change stop は、TUI 起点では `DynamicQueue` の stop フラグを立てて実行側が後から観測する設計であり、WebUI 側は `stop-and-dequeue` API を持つものの、change ID で in-flight の child process を強制終了する共通 backend 契約が明確ではない。tumf の要求は frontend から「指定した実行中の agent command を止める」ことであり、change 単位では cooperative stop ではなく強制 kill が必要である。

## Goals / Non-Goals
- Goals:
  - running change を change ID 単位で強制 kill できること
  - TUI と WebUI が同じ backend force-kill 契約を使うこと
  - kill 完了後に対象 change だけを dequeue / unselect すること
  - serial / parallel の両実行経路で意味が一致すること
- Non-Goals:
  - proposal session cancel の再設計
  - 全体停止 API の置き換え
  - 新しい永続 status 語彙の追加

## Decisions
- Decision: stop-and-dequeue は active change に対して force-kill を保証する
  - Rationale: ユーザー要求が「強制的な停止kill」であり、名前より実動作が重要なため
- Decision: TUI の change 単位 force-kill は `Space` ではなく、`K` で確認に入り `y` で確定する二段階操作にする
  - Rationale: `Space` は queue/unqueue の基本操作として定着しており、active row で即 kill を割り当てると誤操作コストが高すぎるため。さらに確認なし単発 `K` も破壊的操作としては軽すぎるため
- Decision: TUI / WebUI は別々の kill 実装を持たず、change ID ベースの共通 backend kill registry を利用する
  - Rationale: UI ごとに停止挙動がずれるのを防ぐため
- Decision: WebUI の active change 停止も確認ダイアログ必須にする
  - Rationale: TUI と同じく破壊的操作を即実行しない一貫した UX にし、誤クリックによる kill を避けるため
- Decision: dequeue / selected解除は kill 成功を観測してから適用する
  - Rationale: kill 失敗時に UI が完了扱いへ先走る不整合を防ぐため
- Decision: queued だが未起動の change には既存 dequeue を許可し、running change では force-kill を伴う
  - Rationale: waiting item にまで process kill を要求する必要はないため

## Alternatives Considered
- A) 既存の stop flag の観測頻度を上げて疑似即時停止にする
  - Pros: 実装差分が小さい
  - Cons: 外部コマンドがブロック中だと止まらず、要求を満たさない
- B) TUI と WebUI で別々の kill API / command を用意する
  - Pros: それぞれの UI に最適化しやすい
  - Cons: 実装重複と意味のズレが生じやすい

## Risks / Trade-offs
- process handle registry のライフサイクル管理を誤ると、終了済み process への kill や stale handle 参照が起こりうる
  - Mitigation: 実行開始・終了・失敗・cancel の全終端で registry cleanup を行い、idempotent な kill API にする
- 強制 kill と reducer 状態更新の順序が崩れると、not queued 遷移が二重反映される可能性がある
  - Mitigation: state transition は kill 結果イベントからのみ行い、要求時には reducer を先走らせない

## Migration Plan
1. backend に change→execution handle registry を導入
2. serial / parallel 実行が registry へ登録・解除するよう更新
3. TUI / Web API / WebUI を共通 force-kill 経路へ接続
4. 成功・失敗・未起動の回帰テストを追加

## Open Questions
- running row に停止中の一時表示を追加するかは別 change に分離する
