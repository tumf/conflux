## Implementation Tasks

- [ ] 1.1 `dashboard/src/api/restClient.ts` に `addProject(remoteUrl: string, branch: string): Promise<void>` を追加する（`POST /api/v1/projects` に `{ remote_url, branch }` を送信し 201 を受け取ること）
- [ ] 1.2 `dashboard/src/components/AddProjectDialog.tsx` を新規作成する（`remote_url` と `branch` の入力フォームを持つモーダルダイアログ、`DeleteDialog.tsx` と同じスタイルパターンに準拠）
- [ ] 1.3 `dashboard/src/components/ProjectsPanel.tsx` に「+ Add Project」ボタンを追加する（プロジェクト一覧の上部に固定配置、`onAddProject` プロパティでコールバックを受け取る）
- [ ] 1.4 `dashboard/src/App.tsx` に `handleAddProject` ハンドラーと `AddProjectDialog` を追加する（成功時 `toast.success`、失敗時 `toast.error`）
- [ ] 1.5 `cd dashboard && npm run build` がエラーなく完了することを確認する

## Future Work

- E2E テスト（実際のサーバーへの接続が必要）
- 入力フォームのクライアント側バリデーション強化（URL フォーマットチェック等）
