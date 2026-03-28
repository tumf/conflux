## Implementation Tasks

- [ ] 1.1 `App.tsx` のデスクトップサイドバー PROJECTS ヘッダー行を `flex items-center justify-between` に変更し、右端に `Plus` アイコンボタンを追加 (verification: `dashboard/src/App.tsx` L217-219 付近に `<button>` + `<Plus>` が存在)
- [ ] 1.2 `ProjectsPanel.tsx` から「Add Project」ボタン（L31-37）を削除 (verification: `ProjectsPanel.tsx` に `Add Project` テキストが存在しない)
- [ ] 1.3 `ProjectsPanelProps` から `onAddProject` を削除し、呼び出し元の props 渡しも更新 (verification: `ProjectsPanel` の型定義に `onAddProject` がない、`App.tsx` の `panelProps` から除外)
- [ ] 1.4 モバイルレイアウト対応：`ProjectsPanel` にオプショナルな `onAddProject` を残すか、モバイルタブ領域に `+` を追加 (verification: モバイル画面でプロジェクト追加可能)
- [ ] 1.5 ビルド確認 (`cd dashboard && npm run build`) (verification: ビルド成功)
