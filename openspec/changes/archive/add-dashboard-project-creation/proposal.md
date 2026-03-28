# Change: ダッシュボードからプロジェクト追加機能を実装

**Change Type**: implementation

## Why

サーバーモードダッシュボードにプロジェクト追加UIが存在しないため、ユーザーはダッシュボードからプロジェクトを登録できない。バックエンドの `POST /api/v1/projects` エンドポイントは完全実装済みだが、フロントエンドに対応するフォームUI・REST クライアント関数が欠落している。

## What Changes

- `dashboard/src/api/restClient.ts` に `addProject(remoteUrl, branch)` 関数を追加
- `dashboard/src/components/AddProjectDialog.tsx` を新規作成（モーダルダイアログ、`remote_url` と `branch` の入力フォーム）
- `App.tsx` に「+ Add Project」ボタンと `handleAddProject` コールバックを追加
- `ProjectsPanel` に「+ Add Project」ボタンを表示する（空一覧時・非空時ともに）
- spec delta: `server-mode` に「ダッシュボードUIからのプロジェクト追加」要件を追加

## Impact

- Affected specs: `server-mode`
- Affected code: `dashboard/src/api/restClient.ts`, `dashboard/src/components/AddProjectDialog.tsx`, `dashboard/src/components/ProjectsPanel.tsx`, `dashboard/src/App.tsx`

## Out of Scope

- バックエンド API の変更（変更不要）
- 入力値のサーバー側バリデーション強化（既存のまま）
