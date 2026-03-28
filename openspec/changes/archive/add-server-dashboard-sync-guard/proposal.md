# Change: ダッシュボードでresolve_command未設定時にSyncボタンを無効化する

## Why
`POST /api/v1/projects/:id/git/sync` は `resolve_command` が設定されていないと即座に 422 エラーを返す。ダッシュボードのSyncボタンはこの前提条件を事前にチェックせず、ユーザーは常にエラートーストを見ることになる。

## What Changes
- サーバー側: `GET /api/v1/projects/state` のレスポンスに `sync_available: bool` を追加（`resolve_command` が設定されているかどうか）
- WebSocket `full_state` メッセージにもトップレベルで `sync_available: bool` を含める
- ダッシュボード: `sync_available === false` の場合、Syncボタンを `disabled` にする

## Impact
- Affected specs: server-mode
- Affected code: `src/server/api.rs`, `src/remote/types.rs`, `dashboard/src/api/types.ts`, `dashboard/src/components/ProjectCard.tsx`, `dashboard/src/store/useAppStore.ts`
