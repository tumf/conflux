---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/proposal-session-ui/spec.md
  - dashboard/src/components/ChatInput.tsx
  - dashboard/src/components/ProposalChat.tsx
  - dashboard/src/hooks/useProposalChat.ts
---

# Change: proposal chat の送信ロックを user_message ACK までに限定する

**Change Type**: implementation

## Problem / Context

webui server mode の proposal chat は、送信可否をチャット全体の turn status (`ready` / `submitted` / `streaming` / `recovering` / `error`) に結びつけている。その結果、assistant 応答中や recovery 中も送信ボタンが無効化され続け、一般的なチャット UI と比べて過剰に厳しいロックになっている。

現行実装では、送信直後に input を即座にクリアし、その後の `turn_complete` まで送信を再開できない。ユーザー要望はこれより単純で、送信から `user_message` ACK 受信までだけ input をロックし、ACK 到着時に input をクリアして送信ボタンを再有効化することに限定される。

## Proposed Solution

proposal chat の送信ロックを active turn lifecycle から切り離し、クライアント送信から対応する `user_message` ACK 到着までだけに限定する。

- 送信時に input と send button をロックする
- ローカル送信時点では input をクリアしない
- 対応する `user_message` ACK を受信した時点で input をクリアし、send button を再有効化する
- assistant streaming、tool call、turn completion、recovery 状態は送信ロック条件に使わない
- rapid double-submit 防止は ACK 待ちの submission lock で維持する

## Acceptance Criteria

- メッセージ送信後、対応する `user_message` ACK を受けるまでだけ input と send button が無効化される
- `user_message` ACK 受信時に input がクリアされ、send button が即座に再有効化される
- assistant の streaming 中でも、ACK 済みであれば次の送信が可能になる
- 既存の optimistic message 相関 (`client_message_id`) は維持される
- 送信ロックの仕様が `proposal-session-ui` spec delta に明文化される

## Out of Scope

- proposal session の streaming 表示仕様全体の再設計
- disconnected 時の pending queue / retry UX の再設計
- backend WebSocket メッセージ形式の変更
