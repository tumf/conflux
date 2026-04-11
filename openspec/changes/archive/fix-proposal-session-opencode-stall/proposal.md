---
change_type: implementation
priority: high
dependencies: []
references:
  - src/server/api/proposals.rs
  - src/server/acp_client.rs
  - tests/e2e_proposal_session.rs
  - openspec/specs/proposal-session-backend/spec.md
  - openspec/specs/proposal-ws-streaming/spec.md
---

# Proposal: fix-proposal-session-opencode-stall

**Change Type**: implementation

## Problem / Context

サーバーモードの WebUI proposal session で、opencode が応答中にもかかわらずクライアント側のストリーミングが停止したように見えるケースがある。

現状の backend 実装では、proposal session WebSocket 接続ごとに ACP notification relay task を起動するが、接続終了時に残タスクの停止を保証していない。また、ACP request/response 相関は単一 receiver に依存し、並行 request で想定外 ID の response を破棄しうる。これにより reconnect 後の通知取りこぼしや、prompt/cancel/elicitation が重なった時の response timeout が発生し、WebUI からは opencode の応答停止として観測される。

## Proposed Solution

proposal session backend の通信健全性を強化する。

- WebSocket 接続終了時に notification/send/receive task を確実に終了し、旧接続が notification receiver を握り続けないようにする
- ACP response 相関を request ID ごとの pending waiter に変更し、並行 request でも response を正しい caller に配送する
- ACP stdout の JSON-RPC 判定を整理し、response / notification / 未対応 server request を明示的に扱う
- reconnect と並行 request を含む回帰テストを追加し、proposal session のストリーミング継続性を検証する

## Acceptance Criteria

- reconnect 後の proposal session WebSocket は、旧接続の残タスクに妨げられず `session/update` を受信し続ける
- 並行する ACP request があっても、response は request ID ごとに正しく相関され、想定外 ID の破棄による timeout を起こさない
- ACP stdout parser は `method` の有無を基準に notification / response を区別し、未対応の server request や非 u64 response id をログで可観測化する
- proposal session の既存 UX（prompt, cancel, elicitation, reconnect replay）は維持される
- 関連 unit/integration/e2e テストで上記回帰が検証される

## Out of Scope

- proposal session UI デザイン変更
- ACP プロトコル自体の拡張
- opencode 側実装の変更
