---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/proposal-session-ui/spec.md
  - openspec/specs/proposal-session-backend/spec.md
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/hooks/useProposalWebSocket.ts
  - dashboard/src/store/useAppStore.ts
  - src/server/api.rs
  - src/server/proposal_session.rs
---

# Change: チャット履歴復元をWSリプレイに一本化し、二重復元バグを修正

**Change Type**: implementation

## Why

ダッシュボードのProposalチャットに3つの関連バグがある:

1. **発言が消える**: リロードや再接続時にメッセージが消失する
2. **発言がマージされる**: 複数メッセージが1つに統合されてしまう
3. **入力欄がdisableのまま**: 送信後に入力が復活しない

根本原因は **REST API hydrate（`listProposalSessionMessages`）** と **WS接続時リプレイ** が同時に履歴を復元しようとする二重復元問題。サーバーはOpenCodeセッションのインメモリ `message_history` を Source of Truth として保持しているため、WS リプレイ一本に統一するのが正しい。

## What Changes

### サーバー側（Rust）
- WS接続時リプレイループ（`api.rs`）で `user` ロールのメッセージも送信する。新しいWSメッセージ種別 `user_message` を追加
- `ProposalWsServerMessage` enum に `UserMessage` バリアントを追加

### フロント側（TypeScript）
- REST API hydrate（`ProposalChat.tsx` の `useEffect` で `listProposalSessionMessages` を呼ぶ処理）を削除
- `useProposalWebSocket` に `user_message` イベントのハンドリングを追加（新コールバック `onUserMessage`）
- `useProposalWebSocket` の `onclose` で `isAgentResponding` が true のまま残る問題を修正（active turn cleanup コールバック追加）
- WSメッセージ型定義（`api/types.ts`）に `user_message` を追加

## Impact

- Affected specs: `proposal-session-ui`, `proposal-session-backend`
- Affected code: `src/server/api.rs`, `src/server/proposal_session.rs`, `dashboard/src/components/ProposalChat.tsx`, `dashboard/src/hooks/useProposalWebSocket.ts`, `dashboard/src/store/useAppStore.ts`, `dashboard/src/api/types.ts`

## Acceptance Criteria

1. リロード後にWSリプレイ経由でuser/assistantメッセージが全て復元される
2. 複数のメッセージが1つに統合されない（各メッセージが独立して表示される）
3. WS切断後に入力欄が disabled のまま残らない
4. REST API `listProposalSessionMessages` は残すが、フロントからは呼ばれなくなる（デバッグ用途）

## Out of Scope

- REST API `listProposalSessionMessages` の削除（デバッグ用に残す）
- メッセージの永続化（ファイル/DB）— OpenCodeセッションのインメモリ履歴で十分
