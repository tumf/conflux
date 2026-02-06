## 1. Implementation
- [ ] 1.1 `GET /api/worktrees` を実装し、worktree一覧APIを提供する
- [ ] 1.2 Worktree作成エンドポイントを実装する
- [ ] 1.3 Worktree削除エンドポイントを実装し、未マージ状態の削除を `409` で拒否する
- [ ] 1.4 Worktreeマージエンドポイントを実装し、衝突状態のマージを `409` で拒否する
- [ ] 1.5 WebSocket `state_update.worktrees` 同期を実装し、RESTと同一スナップショットを配信する
- [ ] 1.6 WebダッシュボードにWorktreesパネルを追加し、一覧表示と操作UIを実装する
- [ ] 1.7 OpenAPI定義とREADMEを更新し、追加APIとエラー契約を明記する
- [ ] 1.8 全Worktree操作に構造化ログを追加し、失敗時コンテキストを記録する
