## Context

proposal session の chat contract は backend / UI / integration spec に分散しており、履歴 hydration と送信状態管理の canonical source が競合している。現状実装は REST で履歴を先に hydrate し、その後 WebSocket で replay を受けるが、今回の合意では履歴復元を WebSocket-only に統一する。また、送信 UX はサーバ ACK まで入力欄ごと lock し、送信失敗時には failed 表示と retry を提供する必要がある。

## Goals / Non-Goals

- Goals:
  - proposal session の初期履歴復元と reconnect recovery を WebSocket replay/recovery に一本化する
  - 送信中 lock と failed/retry の責務を hook と UI で明確化する
  - canonical spec と Dashboard 実装の契約を一致させる
- Non-Goals:
  - proposal session persistence schema の刷新
  - proposal session 以外の chat surface への横展開
  - WebSocket transport 自体を別プロトコルへ置き換えること

## Decisions

- Decision: proposal session の履歴復元は REST baseline ではなく WebSocket replay/recovery を唯一の hydration source とする
  - Alternatives considered:
    - REST baseline + WebSocket 増分: rejected because canonical spec を単純化したいという今回の整理方針と衝突し、二重 source による重複排除責務を増やす
    - in-memory のみで replay し永続 history を使わない: rejected because server restart 後の復元契約を弱める

- Decision: 入力欄と送信ボタンは user message ACK 受信まで lock する
  - Alternatives considered:
    - textarea を常時 editable にする: rejected because今回の合意と衝突し、pending prompt と草稿編集の責務が混ざる
    - transport send 完了時点で unlock する: rejected because user-visible ACK とズレる

- Decision: 送信失敗は queued pending のまま放置せず `failed` state と explicit retry action に遷移させる
  - Alternatives considered:
    - 自動再送のみで failed UI を出さない: rejected becauseユーザに失敗が観測できず、再送制御も曖昧になる

## Risks / Trade-offs

- WebSocket-only hydration では replay 契約の idempotency がより重要になり、message_id / turn_id の安定性に依存する
- pending prompt queue と failed retry を両立させるため、hook の状態機械がやや複雑になる
- 既存 REST history テストは期待値を書き換える必要がある

## Migration Plan

1. spec delta を統一し、REST hydration baseline 要件を削除する
2. Dashboard hook から `listProposalSessionMessages` 依存を除去し、WebSocket replay 前提の初期化へ切り替える
3. failed/retry state transition を実装し、関連テストを replay/reconnect と合わせて更新する
4. lint / test / rust quality gate で回帰を確認する

## Open Questions

- REST の `/messages` endpoint を完全に廃止するか、debug/admin 用 read API として残すかは実装時に最小変更で判断する
