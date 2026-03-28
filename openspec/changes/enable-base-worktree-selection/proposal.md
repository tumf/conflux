# Change: WorktreesPanelでベース(main) worktreeを選択可能にし先頭に表示する

## Problem / Context

Webダッシュボードの Worktrees タブで、`is_main === true` の worktree（ベースリポジトリ）がクリック不可になっている。`WorktreeRow.tsx` の `handleRowClick` が `is_main` をガードしているため、ベースのファイルツリーを右パネルで閲覧できない。

また、worktree 一覧の表示順がバックエンドから返される順序そのままで、ベース worktree が必ず先頭に来る保証がない。

## Proposed Solution

1. **WorktreeRow のクリック制限を撤廃**: `is_main` ガードを外し、全 worktree を選択可能にする
2. **WorktreesPanel でベース worktree を先頭にソート**: `is_main === true` のエントリを常に一覧の先頭に表示する
3. **base worktree 選択時のファイルツリー表示**: `fileBrowseContext` に `{ type: 'worktree', worktreeBranch: branch }` をセットし、`FileViewPanel` で `root=worktree:{branch}` としてファイルツリーを取得する

## Acceptance Criteria

- Worktrees タブでベース worktree をクリックすると右パネルの Files タブにベースのファイルツリーが表示される
- ベース worktree は常に一覧の先頭に表示される
- ベース worktree にカーソルポインタが表示される
- 他の worktree の既存動作（クリック、マージ、削除）に影響がない
- `cargo test` が全て通る

## Out of Scope

- バックエンド API の変更
- `FileBrowseContext` 型への新しい `type` の追加（既存の `worktree` type をそのまま使う）
- モバイルレイアウトの変更（既存のモバイル対応をそのまま維持）
