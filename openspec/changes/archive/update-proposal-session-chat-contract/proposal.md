---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/specs/proposal-session-backend/spec.md
  - openspec/specs/proposal-session-ui/spec.md
  - openspec/specs/proposal-session-integration/spec.md
  - dashboard/src/hooks/useProposalChat.ts
  - dashboard/src/components/ChatInput.tsx
  - src/server/api/proposals.rs
---

# Update: Canonicalize proposal session chat contract

**Change Type**: hybrid

## Premise / Context

- ユーザは `openspec/specs` の欠陥整理に加えて、仕様通りに実装されていない箇所も修正したいと明示している
- 現行 canonical spec では proposal session の履歴 hydration 契約が `WebSocket-only` と `REST baseline` の両方で記述されており衝突している
- セッション中の合意として、履歴復元は `WebSocket-only`、送信 UX は「サーバ ACK まで入力欄ごと disable、失敗時は retry」を採用する
- 実装は `dashboard/src/hooks/useProposalChat.ts` で依然として `listProposalSessionMessages()` による REST hydration を行っており、canonical 契約と一致していない
- UI 側には `ChatInput.tsx` の入力ロックと `ChatMessageList.tsx` の pending/failed 表示がある一方、失敗送信を明示的に `failed` へ遷移させる hook 契約は未整理で、spec と実装の責務境界も曖昧である

## Problem / Context

proposal session の chat contract が canonical spec 内で競合しているため、Dashboard と backend の責務分担を一意に解釈できない。

特に以下が問題である。

1. backend spec は「履歴復元は WebSocket replay のみ」と「REST history が hydration baseline」の両方を要求している
2. UI spec は送信中入力を disable する旧方針と、textarea は常に editable とする新方針が共存している
3. 実装は REST history を先に読み込んでから WebSocket 接続するため、採用済みの `WebSocket-only` 方針と一致しない
4. pending / failed / retry の送信状態契約が spec 間で十分に固定されていない

このままでは proposal session の reconnect・replay・retry 実装を安定して保守できない。

## Proposed Solution

proposal session chat contract を 1 つの canonical 方針に整理し、その方針に合わせて Dashboard / backend 実装を修正する。

具体的には以下を行う。

1. `proposal-session-backend` / `proposal-session-ui` / `proposal-session-integration` の spec を `WebSocket-only hydration` 前提に統一する
2. 「送信中はサーバ ACK まで入力欄ごと disable」「失敗時は failed 表示と retry」を canonical UI 契約として固定する
3. Dashboard から REST history hydration (`listProposalSessionMessages`) 依存を除去し、初期履歴・再接続復元を WebSocket replay/recovery に一本化する
4. send failure / reconnect / retry の state transition を hook と UI の責務として明示し、対応テストを揃える

## Acceptance Criteria

1. proposal session の canonical spec から `REST hydration baseline` 要件が除去され、履歴復元は WebSocket replay/recovery のみと読める
2. proposal session UI spec が「送信中は入力欄ごと disable、ACK で解除、送信失敗時は failed + retry」に統一される
3. `dashboard/src/hooks/useProposalChat.ts` が session 初期化時に `listProposalSessionMessages()` を使わず、WebSocket replay のみで履歴を復元する
4. Dashboard の送信失敗時に user message が `failed` として表示され、Retry 操作で再送できる実装とテストがある
5. `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate update-proposal-session-chat-contract --strict` が成功する
6. 実装時の品質確認として `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `npm --prefix dashboard run lint`, `npm --prefix dashboard run test -- --runInBand` 相当の repository-verifiable quality gate が tasks に記載される

## Out of Scope

- proposal session 以外の dashboard chat UI の全面 redesign
- proposal session message persistence schema の再設計
- mobile drawer や markdown renderer など今回の契約整理と無関係な UI 改修
