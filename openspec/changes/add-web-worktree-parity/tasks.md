## 1. Implementation
- [x] 1.1 TUI/Web共通のworktree取得・ガード判定ロジック（削除可否、マージ可否、対象存在確認）を抽出する
- [x] 1.2 `src/web/api.rs` のworktree APIハンドラを有効化し、`src/web/mod.rs` でルート登録して `/api/worktrees*` を提供する
- [x] 1.3 `/api/state` と `state_update.worktrees` に最新worktreeスナップショットを統合する
- [x] 1.4 未マージworktreeの削除拒否を `src/worktree_ops.rs::can_delete_worktree` に実装する
- [x] 1.5 構造化ログ（`request_id`, `operation`, `worktree_name`, `error`, `duration_ms`）を全worktree操作ハンドラに追加する
- [x] 1.6 WebダッシュボードにWorktrees Viewを実装（タブ切り替え、一覧表示、操作ガード、削除確認ダイアログ）する

## Future Work
- APIハンドラ/状態同期/UIのテストを追加し、主要成功系と拒否系を網羅する
- OpenAPI定義とREADMEを更新し、一覧・操作API、ステータスコード、WS同期要件を明記する

## Acceptance #1 Failure Follow-up
- [x] 作業ツリーをクリーンにする（未コミット変更: `openspec/changes/add-web-worktree-parity/tasks.md`, `src/main.rs`, `src/web/api.rs`, `src/web/mod.rs`, `src/web/state.rs`, `src/worktree_ops.rs`, `web/app.js`, `web/index.html`, `web/style.css`）。
- [x] Worktree操作の構造化ログが全経路で `request_id`, `operation`, `worktree_name`, `error`, `duration_ms` を満たすように整理する（`src/web/api.rs::list_worktrees` など）。
- [x] Worktree操作失敗時のログ欠落を解消する（`src/web/api.rs::create_worktree` の設定/基準コミット取得失敗、`merge_worktree` の404/409/checkout失敗、`execute_worktree_command` の404など）。
- [x] `state_update.worktrees` が `/api/state` と同期されるよう、`src/web/state.rs::refresh_from_disk` の更新・ブロードキャスト順序を修正する。
