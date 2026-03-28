## ADDED Requirements

### Requirement: dashboard-file-viewer

サーバーモードダッシュボードは、右ペインからプロジェクトのファイルをブラウズして閲覧できなければならない。

#### Scenario: desktop-right-pane-switches-between-logs-and-files

**Given**: デスクトップレイアウトでプロジェクトが選択されている
**When**: ユーザーが右ペインの `Files` タブを選択する
**Then**: 右ペインは `Logs` 表示から `Files` 表示に切り替わる
**And**: `Logs` タブに戻すとログ表示に戻る

#### Scenario: change-click-opens-project-tree-with-change-expanded

**Given**: プロジェクトが選択され、change `add-feature` が表示されている
**When**: ユーザーが change `add-feature` 行（チェックボックス以外の領域）をクリックする
**Then**: ファイルビューアはプロジェクトルートからのファイルツリーを表示する
**And**: `openspec/changes/add-feature/` までのディレクトリが自動展開される
**And**: `proposal.md` が自動的に選択され、右ペインにその内容が表示される

#### Scenario: worktree-click-opens-worktree-root

**Given**: プロジェクトが選択され、worktree `cflx/add-feature` が表示されている
**When**: ユーザーが worktree 行（アクションボタン以外の領域）をクリックする
**Then**: ファイルビューアはそのworktreeのルートからファイルツリーを表示する

#### Scenario: files-view-shows-placeholder-without-context

**Given**: プロジェクトは選択されているが、changeもworktreeも選択されていない
**When**: ユーザーが `Files` タブを開く
**Then**: 画面は `Select a change or worktree to browse files` などのプレースホルダーを表示する

#### Scenario: mobile-layout-exposes-files-tab

**Given**: モバイルレイアウトでダッシュボードを表示している
**When**: ユーザーがタブバーを見る
**Then**: `Files` タブが表示される
**And**: そのタブからファイルビューアを開ける
