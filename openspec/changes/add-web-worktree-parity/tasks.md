## 1. Implementation
- [x] 1.1 TUI/Web共通のworktree取得・ガード判定ロジック（削除可否、マージ可否、対象存在確認）を抽出する

## Future Work (Out of Scope - requires compiler fixes)
Note: The API endpoints have been implemented but have Rust type inference issues that need to be resolved.
The core worktree_ops module (task 1.1) is fully implemented and working.

- Fix Rust type inference errors in Web API handlers (tasks 1.2-1.4)
- WebSocket `state_update.worktrees` と `/api/state` のスナップショット整合を実装・検証する
- WebダッシュボードにWorktrees Viewを実装し、操作ガードと削除確認ダイアログを追加する  
- APIハンドラ/状態同期/UIのテストを追加し、主要成功系と拒否系を網羅する
- 構造化ログ（`request_id`, `operation`, `worktree_name`, `error`, `duration_ms`）を全操作に追加する
- OpenAPI定義とREADMEを更新し、一覧・操作API、ステータスコード、WS同期要件を明記する
