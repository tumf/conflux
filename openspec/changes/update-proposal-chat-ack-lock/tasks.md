## Implementation Tasks

- [ ] 1. `proposal-session-ui` の送信ロック要件を、turn lifecycle ではなく `user_message` ACK 待ちに基づく仕様へ更新する (verification: spec delta に ACK ベースの requirement/scenario が追加されている)
- [ ] 2. `useProposalChat` の送信可否管理を、`submitted/streaming/recovering/error` ではなく pending submit / ACK 待ちに基づく状態へ整理する (verification: `dashboard/src/hooks/useProposalChat.ts` で `user_message` ACK 受信時に送信ロック解除され、assistant streaming が送信可否に影響しない)
- [ ] 3. `ChatInput` を ACK 受信まで input を保持したままロックし、ACK で clear する挙動へ更新する (verification: `dashboard/src/components/ChatInput.tsx` で送信時即clearがなくなり、ACK時clearになる)
- [ ] 4. proposal chat のコンポーネントテスト / hook テストを、ACK ベースのロックと再有効化へ更新する (verification: `ChatInput.test.tsx` と `useProposalChat.test.ts` に ACK 前ロック・ACK 後再有効化・streaming 中送信可の検証がある)
- [ ] 5. dashboard の lint / typecheck / test を実行して回帰がないことを確認する (verification: dashboard 向け lint・typecheck・test コマンドが成功する)

## Future Work

- disconnected 時の pending / retry UX を、この簡素化後の送信ロックモデルに合わせて再評価する
- recovery 状態の表示責務を送信可否から完全分離する設計見直し
