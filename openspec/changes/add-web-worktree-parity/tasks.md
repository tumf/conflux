## 1. Implementation
- [ ] 1.1 TUI/Web共通のworktree取得・ガード判定ロジック（削除可否、マージ可否、対象存在確認）を抽出する
- [ ] 1.2 `GET /api/worktrees` を実装し、一覧スナップショットを返す
- [ ] 1.3 操作API（`POST /api/worktrees/refresh`, `POST /api/worktrees/create`, `POST /api/worktrees/delete`, `POST /api/worktrees/merge`, `POST /api/worktrees/command`）を実装する
- [ ] 1.4 fail-fastエラー契約を実装する（未マージ削除 `409`、衝突マージ `409`、対象なし `404`、VCS/内部失敗 `500`）
- [ ] 1.5 WebSocket `state_update.worktrees` と `/api/state` のスナップショット整合を実装・検証する
- [ ] 1.6 WebダッシュボードにWorktrees Viewを実装し、操作ガードと削除確認ダイアログを追加する
- [ ] 1.7 APIハンドラ/状態同期/UIのテストを追加し、主要成功系と拒否系を網羅する
- [ ] 1.8 構造化ログ（`request_id`, `operation`, `worktree_name`, `error`, `duration_ms`）を全操作に追加する
- [ ] 1.9 OpenAPI定義とREADMEを更新し、一覧・操作API、ステータスコード、WS同期要件を明記する
