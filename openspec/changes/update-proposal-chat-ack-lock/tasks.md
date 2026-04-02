## Implementation Tasks

- [x] 1. `proposal-session-ui` の送信ロック要件を、turn lifecycle ではなく `user_message` ACK 待ちに基づく仕様へ更新する (verification: spec delta に ACK ベースの requirement/scenario が追加されている)
- [x] 2. `useProposalChat` の送信可否管理を、`submitted/streaming/recovering/error` ではなく pending submit / ACK 待ちに基づく状態へ整理する (verification: `dashboard/src/hooks/useProposalChat.ts` で `user_message` ACK 受信時に送信ロック解除され、assistant streaming が送信可否に影響しない)
- [x] 3. `ChatInput` を ACK 受信まで input を保持したままロックし、ACK で clear する挙動へ更新する (verification: `dashboard/src/components/ChatInput.tsx` で送信時即clearがなくなり、ACK時clearになる)
- [x] 4. proposal chat のコンポーネントテスト / hook テストを、ACK ベースのロックと再有効化へ更新する (verification: `ChatInput.test.tsx` と `useProposalChat.test.ts` に ACK 前ロック・ACK 後再有効化・streaming 中送信可の検証がある)
- [x] 5. dashboard の lint / typecheck / test を実行して回帰がないことを確認する (verification: `npm run lint` / `npx tsc --noEmit --jsx react-jsx --module esnext --moduleResolution bundler --target es2022 --lib dom,es2022 --skipLibCheck src/hooks/useProposalChat.ts src/components/ChatInput.tsx src/components/ProposalChat.tsx src/components/__tests__/ChatInput.test.tsx src/hooks/useProposalChat.test.ts` / `npm run test -- --run` / `npm run build` が成功する)

## Acceptance #1 Failure Follow-up

- [x] `ChatInput.test.tsx` の送信後クリア挙動テストを新仕様（ACK まで input 保持、`clearVersion` 増分でクリア）に合わせて修正する
- [x] `ChatInput.test.tsx` 内の旧 `status` prop 指定を新 API (`isSubmissionLocked` / `clearVersion`) に更新する

## Acceptance #2 Failure Follow-up

- [x] dashboard の typecheck 手順を実在する設定・コマンドに合わせて修正し、成功ログを再取得する (`dashboard/` には `tsconfig*.json` が存在しないため、変更対象の proposal chat 関連 TS/TSX ファイルを `npx tsc --noEmit --jsx react-jsx --module esnext --moduleResolution bundler --target es2022 --lib dom,es2022 --skipLibCheck ...` で型検証)
- [x] task 5 の verification 記述を、実際にこのリポジトリで成功確認できる lint / typecheck / test / build コマンドへ更新する

## Future Work

- disconnected 時の pending / retry UX を、この簡素化後の送信ロックモデルに合わせて再評価する
- recovery 状態の表示責務を送信可否から完全分離する設計見直し
