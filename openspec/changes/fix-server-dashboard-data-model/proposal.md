# Change: サーバーモードダッシュボードのデータモデル不一致を修正する

## Why
サーバーモード (`cflx server`) のWebダッシュボード (`/dashboard`) が動作しない。根本原因は、サーバーAPIが返す `RemoteProject` のスキーマとダッシュボードフロントエンドが期待するスキーマが一致していないため。具体的には:
- サーバーは `name` (e.g. "repo@branch") を返すが、フロントは `repo` と `branch` を別フィールドで期待
- サーバーは `status` / `is_busy` / `error` を `RemoteProject` に含めない
- `changes` はサーバー側でプロジェクトにネストされるが、フロントは別配列として管理
- `RemoteChange` のフィールド名が不一致 (`project` vs `project_id`)

## What Changes
- サーバー側 `RemoteProject` に `repo`, `branch`, `status`, `is_busy`, `error` フィールドを追加
- `build_remote_project_snapshot_async` で新フィールドを `ProjectEntry` から設定
- ダッシュボード `types.ts` をサーバーの新スキーマに合わせて更新
- ダッシュボード `wsClient.ts` で `full_state` メッセージからネストされた `changes` をフラット化
- `ChangesPanel.tsx` の `project_id` → `project` フィールド名を修正

## Impact
- Affected specs: server-mode
- Affected code: `src/remote/types.rs`, `src/server/api.rs`, `dashboard/src/api/types.ts`, `dashboard/src/api/wsClient.ts`, `dashboard/src/components/ChangesPanel.tsx`, `dashboard/src/store/useAppStore.ts`
