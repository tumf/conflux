# Change: Proposal Session End-to-End Integration Verification

## Problem / Context

`add-proposal-session-backend` と `add-proposal-session-ui` は独立して実装される。両方完了後に、バックエンド API ↔ Dashboard UI ↔ ACP Agent の結合が正しく動作することを検証し、不整合やエッジケースを修正する必要がある。

## Dependencies

- `add-proposal-session-backend`
- `add-proposal-session-ui`

## Proposed Solution

E2Eテストと統合テストを追加し、実際の `opencode acp` (またはモック ACP バイナリ) を使ったフルフロー検証を行う。発見された不整合はこの change 内で修正する。

## Acceptance Criteria

- E2Eテスト: セッション作成 → プロンプト送信 → エージェント応答受信 → change検出 → コミット → マージ → セッションクリーンアップ の一連のフローが成功する
- E2Eテスト: Elicitation リクエスト → UI表示 → ユーザー応答 → エージェント継続 のフローが成功する
- E2Eテスト: dirty worktree でのセッションクローズ警告 → 強制クローズが正しく動作する
- E2Eテスト: 複数セッション同時操作（作成・切替・独立した会話）が正しく動作する
- WebSocketメッセージ型がバックエンドとフロントエンドで一致している
- 非活動タイムアウト後のUI側ハンドリング（再接続プロンプトまたはエラー表示）が動作する
- `cargo test` と `cd dashboard && npm run test && npm run build` が全て通る

## Out of Scope

- パフォーマンス負荷テスト
- ACP URL-mode elicitation
