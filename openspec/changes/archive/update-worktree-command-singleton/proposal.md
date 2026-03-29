# Change: server-mode WebUI の worktree 単位コマンド実行管理

## Why
server-mode WebUI の Sync ボタンはブラウザリロード後に状態が失われ、同じ base/worktree に対する別コマンドとの競合も防げていない。特に base worktree で Sync 実行中に別の base 向けコマンドを開始できる状態は、UI 表示の不整合だけでなく Git 操作の競合を招く。

## What Changes
- server-mode に worktree root（base を含む）単位の active command 管理を追加する
- busy な root への新規コマンド要求は待機させず、即時 `409 Conflict` で拒否する
- WebSocket `full_state` に active command 情報を含め、ダッシュボードがリロード後も busy 状態を復元できるようにする
- dashboard の Sync ボタンと worktree 操作ボタンを active command 状態に従って disable / 進行表示する
- `git/sync` 内部の resolve_command を含む出力をプロジェクトログへ配信する

## Acceptance Criteria
- 同一 base root に対して Sync 実行中は、同 root 向けの別コマンド要求が即時 `409 Conflict` で失敗する
- 同一 worktree root に対して apply / merge / delete など別コマンドが同時に開始されない
- dashboard をリロードしても、実行中 root のボタン disable と進行表示が維持される
- Sync 実行中の resolve_command stdout/stderr が対象プロジェクトのログパネルに表示される
- active command 状態はサーバー再起動では保持されず、サーバー稼働中の真実源として扱われる

## Out of Scope
- サーバー再起動をまたぐジョブ永続化
- busy root への待機キューや自動再試行
- project 単位での追加の広域排他ルール導入

## Impact
- Affected specs: `server-mode`, `server-mode-dashboard`, `server-api`
- Affected code: `src/server/api.rs`, `src/server/mod.rs`, `src/server/registry.rs` または同等の server 状態管理モジュール、`dashboard/src/App.tsx`, `dashboard/src/store/useAppStore.ts`, `dashboard/src/api/types.ts`, `dashboard/src/components/ProjectCard.tsx`, `dashboard/src/components/WorktreeRow.tsx`
