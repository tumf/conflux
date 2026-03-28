# Change: Add Project ボタンを PROJECTS ヘッダー右端に移動

**Change Type**: implementation

## Why

現在「Add Project」ボタンはプロジェクトリスト内にフル幅の破線ボーダーボタンとして配置されており、リストの一部として扱われている。ヘッダー行に `+` アイコンのみで配置することで、視覚的なノイズを減らし、ヘッダーのスペースを有効活用する。

## What Changes

- デスクトップサイドバーの「PROJECTS」ヘッダー行（`App.tsx`）に `+` アイコンボタンを右寄せで追加
- `ProjectsPanel` コンポーネントから既存の「Add Project」ボタンを削除
- `ProjectsPanelProps` から `onAddProject` を削除（ヘッダー側で直接ハンドリング）
- モバイルレイアウトではタブがヘッダー代替のため、`ProjectsPanel` 内にコンパクトな `+` ボタンをフォールバック表示

## Impact

- Affected code: `dashboard/src/App.tsx`, `dashboard/src/components/ProjectsPanel.tsx`
- 既存の `AddProjectDialog` は変更なし

## Acceptance Criteria

1. デスクトップサイドバーで「PROJECTS」テキストの右端に `+` アイコンのみのボタンが表示される
2. `+` ボタンクリックで `AddProjectDialog` が開く
3. ホバー時にインディゴ色 (`#6366f1`) に変化する
4. `ProjectsPanel` のプロジェクトリスト上部に旧ボタンが表示されない
5. モバイルレイアウトでもプロジェクト追加機能にアクセスできる

## Out of Scope

- `AddProjectDialog` の内容やデザインの変更
- プロジェクトカードのデザイン変更
