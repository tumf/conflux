---
change_type: implementation
priority: high
dependencies:
  - update-proposal-ws-turn-recovery
references:
  - openspec/specs/proposal-session-ui/spec.md
  - openspec/specs/proposal-session-backend/spec.md
  - dashboard/src/hooks/useProposalChat.ts
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/api/restClient.ts
  - src/server/api.rs
  - src/server/proposal_session.rs
---

# Change: Fix proposal chat session consistency in server mode WebUI

**Change Type**: implementation

## Problem / Context

server mode WebUI の proposal chat で、同一回答が複数回表示されたり、セッション切替直後に別セッションの会話が一時的に表示されたりする不整合がある。

現在の proposal chat は、初回表示時に REST で履歴を取得し、その後 WebSocket 接続時にも replay/history を受け取る構成になっている。また、`useProposalChat` のセッション切替時 async 処理には、古い `projectId/sessionId` に紐づく履歴取得やイベント反映を確実に破棄する仕組みが不足している。

この結果として、次のユーザー影響が発生しうる。

- 同じ論理メッセージが二重 hydration により重複表示される
- 旧セッションの遅延レスポンスが新セッション画面に混入する
- リロードや再接続後に表示順・件数・turn 表示が安定しない

## Proposed Solution

proposal chat の履歴復元・再接続・セッション切替を、セッション境界とメッセージ identity を明示した整合的なモデルへ揃える。

この change では次を行う。

- proposal chat の初回 hydration と reconnect replay の責務境界を明文化し、同一履歴が二重に画面へ反映されないようにする
- proposal chat フロントエンドで、古い session に属する非同期履歴取得結果や WebSocket イベントを現在表示中の session state に反映しないことを保証する
- backend replay/event contract で、クライアントが既存 assistant turn や tool call を安定同一視できる identity を保証する
- reload / reconnect / session tab switch のたびに message history が冪等に復元されることを要件化する

## Acceptance Criteria

1. 同一 proposal session を開き直し・再接続・リロードしても、既存 user / assistant message が重複表示されない。
2. proposal session A から B へ切り替えた際、A に属する遅延履歴取得結果やイベントが B の chat list に表示されない。
3. backend replay は client が既存 assistant turn / tool call を既存 message と同一視できる stable identity を提供するか、または初回 hydration と replay の責務を分離して重複反映を防ぐ。
4. reconnect 後の proposal chat は、表示件数・順序・turn 境界が論理的に一貫し、同一論理応答が複数 assistant message に分裂しない。
5. proposal-session UI / backend の仕様とテストが、session isolation と idempotent history restoration を明示的に検証する。

## Out of Scope

- proposal chat 以外の terminal session WebSocket 整合性修正
- proposal-session prompt 内容や agent behavior の変更
- unrelated dashboard state persistence redesign
