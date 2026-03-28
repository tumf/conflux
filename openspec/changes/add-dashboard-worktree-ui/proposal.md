# Change: Server Mode DashboardにWorktree管理UIを追加

**Change Type**: implementation

## Why

Server Mode Dashboard（React）にWorktree操作UIがなく、TUI と機能格差がある。`add-server-worktree-api` で追加されるAPIを利用し、ダッシュボード上でWorktreeの一覧表示・作成・削除・ブランチマージを可能にする。

## What Changes

- `dashboard/src/api/types.ts` に `WorktreeInfo` 型を追加
- `dashboard/src/api/restClient.ts` にWorktree操作用のREST関数を追加
- `dashboard/src/store/useAppStore.ts` に `worktreesByProjectId` 状態を追加
- `dashboard/src/hooks/useWebSocket.ts` でWebSocketの `full_state` からworktree情報を取得
- `WorktreesPanel.tsx` コンポーネント: Worktree一覧表示（ブランチ名、状態バッジ、コンフリクト表示）
- `WorktreeRow.tsx` コンポーネント: 行内アクションボタン（マージ、削除）
- `CreateWorktreeDialog.tsx`: Worktree作成ダイアログ
- `DeleteWorktreeDialog.tsx`: 削除確認ダイアログ
- `App.tsx` のレイアウト変更: Changes/Worktreesタブ切り替え

## Impact

- Affected specs: `web-monitoring`
- Affected code: `dashboard/src/` 配下全般

## Dependencies

- `add-server-worktree-api` が先に実装されている必要がある

## Out of Scope

- Worktree内でのコマンド実行UI
- エディタ起動
