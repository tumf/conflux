## MODIFIED Requirements

### Requirement: worktree-list-selection

Worktrees タブの一覧でベース worktree を含む全エントリが選択可能であること。

#### Scenario: ベース worktree をクリックしてファイルツリーを表示する

**Given**: プロジェクトが選択されており Worktrees タブが表示されている
**When**: ベース worktree（`is_main === true`）の行をクリックする
**Then**: 右パネルが Files タブに切り替わり、ベース worktree のファイルツリーが表示される

### Requirement: worktree-list-ordering

Worktrees タブの一覧でベース worktree が常に先頭に表示されること。

#### Scenario: ベース worktree が先頭に表示される

**Given**: プロジェクトに複数の worktree が存在する
**When**: Worktrees タブを表示する
**Then**: `is_main === true` の worktree が一覧の先頭に表示される
