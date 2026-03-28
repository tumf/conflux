## Implementation Tasks

- [ ] Task 1: WorktreeRow の `is_main` クリックガードを撤廃する (`dashboard/src/components/WorktreeRow.tsx:26-29`) — `handleRowClick` から `if (!worktree.is_main)` ガードを削除し、39行目の `cursor-pointer` 条件も常時適用に変更する (verification: ブラウザでベース worktree をクリックして右パネルにファイルツリーが表示されること)
- [ ] Task 2: WorktreesPanel でベース worktree を先頭にソートする (`dashboard/src/components/WorktreesPanel.tsx:68-79`) — `worktrees` 配列を表示前にソートし、`is_main === true` のエントリを先頭に配置する (verification: `npm run build` が成功し、Worktrees タブでベース worktree が常に先頭に表示されること)
- [ ] Task 3: dashboard の build が成功することを確認する (verification: `cd dashboard && npm run build` がエラーなく完了すること)
