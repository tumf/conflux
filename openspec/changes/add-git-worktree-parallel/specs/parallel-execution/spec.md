## ADDED Requirements

### Requirement: VCS Backend Abstraction

システムは並列実行のために VCS バックエンド抽象化レイヤーを提供しなければならない（SHALL）。

`WorkspaceManager` trait は以下の操作を定義する:
- VCS 利用可能性チェック
- ワークスペース作成
- リビジョン取得
- マージ
- クリーンアップ

#### Scenario: JjWorkspaceManager implements trait

- **WHEN** jj リポジトリで並列実行が開始される
- **THEN** `JjWorkspaceManager` が `WorkspaceManager` trait を実装する
- **AND** 既存の jj ベースの並列実行動作は変更されない

#### Scenario: GitWorkspaceManager implements trait

- **WHEN** Git リポジトリで並列実行が開始される
- **THEN** `GitWorkspaceManager` が `WorkspaceManager` trait を実装する
- **AND** Git Worktree を使用してワークスペースを管理する

### Requirement: VCS Backend Auto-Detection

システムは並列実行時に VCS バックエンドを自動検出しなければならない（SHALL）。

検出優先順位:
1. jj リポジトリ（`.jj` ディレクトリ存在）→ jj バックエンド
2. Git リポジトリ（`.git` ディレクトリ存在）→ Git バックエンド
3. 両方なし → 並列実行不可エラー

#### Scenario: Auto-detect jj backend

- **WHEN** カレントディレクトリに `.jj` ディレクトリが存在する
- **AND** `--vcs` オプションが指定されていない、または `auto` である
- **THEN** jj バックエンドが選択される
- **AND** 既存の jj 並列実行動作が使用される

#### Scenario: Auto-detect git backend

- **WHEN** カレントディレクトリに `.jj` ディレクトリが存在しない
- **AND** `.git` ディレクトリが存在する
- **AND** `--vcs` オプションが指定されていない、または `auto` である
- **THEN** Git バックエンドが選択される

#### Scenario: No VCS available

- **WHEN** `.jj` も `.git` も存在しない
- **AND** `--parallel` フラグが指定されている
- **THEN** エラーメッセージ "Parallel mode requires jj or git repository" が表示される
- **AND** 終了コードは非ゼロである

#### Scenario: Explicit VCS selection with --vcs flag

- **WHEN** `--vcs git` が指定されている
- **AND** `.git` ディレクトリが存在する
- **THEN** Git バックエンドが使用される（jj が存在しても）

#### Scenario: Explicit VCS not available

- **WHEN** `--vcs jj` が指定されている
- **AND** `.jj` ディレクトリが存在しない
- **THEN** エラーメッセージ "jj repository not found (.jj directory missing)" が表示される
- **AND** 終了コードは非ゼロである

### Requirement: Git Worktree Workspace Management

Git バックエンド使用時、システムは `git worktree` コマンドを使用してワークスペースを管理しなければならない（SHALL）。

#### Scenario: Create workspace with git worktree

- **WHEN** Git バックエンドでワークスペース作成が要求される
- **THEN** `git worktree add <path> -b <branch> <base_rev>` が実行される
- **AND** 各変更は独立したブランチを持つ
- **AND** ワークスペースパスはシステム一時ディレクトリ配下に作成される

#### Scenario: Cleanup workspace

- **WHEN** Git ワークスペースのクリーンアップが要求される
- **THEN** `git worktree remove <path>` が実行される
- **AND** 関連ブランチ `git branch -D <branch>` が削除される

#### Scenario: Get workspace revision

- **WHEN** Git ワークスペースのリビジョンが要求される
- **THEN** `git rev-parse HEAD` の結果が返される

### Requirement: Git Clean Working Directory Requirement

Git バックエンド使用時、システムは未コミット変更がある場合に並列実行を拒否しなければならない（SHALL）。

#### Scenario: CLI error on uncommitted changes

- **WHEN** `--parallel` フラグで実行される
- **AND** Git バックエンドが選択される
- **AND** 未コミットまたは未追跡のファイルが存在する
- **THEN** 以下のエラーメッセージが表示される:
  ```
  Error: Cannot start parallel mode with uncommitted changes.
  
  Your working directory has uncommitted changes. Git worktree requires
  a clean working directory to create isolated workspaces.
  
  Please resolve this by either:
  
    1. Commit your changes:
       git add -A && git commit -m "WIP: save work before parallel"
  
    2. Stash your changes:
       git stash push -u -m "before parallel execution"
  
  Then run the command again.
  ```
- **AND** 終了コードは非ゼロである

#### Scenario: TUI popup error on uncommitted changes

- **WHEN** TUI で F5 キーが押される
- **AND** Git バックエンドが選択される
- **AND** 未コミットまたは未追跡のファイルが存在する
- **THEN** ポップアップダイアログが表示される
- **AND** タイトルは "Uncommitted Changes Detected" である
- **AND** 本文に解決手順が表示される
- **AND** Enter キーでダイアログを閉じることができる
- **AND** 並列実行は開始されない

#### Scenario: jj behavior unchanged

- **WHEN** jj バックエンドが選択される
- **AND** 未コミット変更が存在する
- **THEN** 従来通り `jj new` でスナップショットが作成される
- **AND** 並列実行が正常に開始される
- **AND** エラーは発生しない

### Requirement: Git Sequential Merge

Git バックエンド使用時、システムは複数ブランチを逐次マージしなければならない（SHALL）。

#### Scenario: Merge single branch

- **WHEN** 1つのワークスペースブランチをマージする
- **THEN** `git merge <branch>` が実行される
- **AND** マージコミットが作成される

#### Scenario: Merge multiple branches sequentially

- **WHEN** 複数のワークスペースブランチをマージする
- **THEN** 各ブランチが1つずつマージされる
- **AND** マージ順序はワークスペース作成順である
- **AND** 各マージ後にコンフリクトがチェックされる

#### Scenario: Conflict detected during merge

- **WHEN** `git merge` がコンフリクトを検出する
- **THEN** `GitConflict` エラーが返される
- **AND** コンフリクトファイルのリストが含まれる
- **AND** AgentRunner によるコンフリクト解決が試行される

### Requirement: Git Conflict Resolution

Git バックエンド使用時、システムは Git コンフリクトマーカーを含む解決プロンプトを提供しなければならない（SHALL）。

#### Scenario: Git conflict resolution prompt

- **WHEN** Git マージでコンフリクトが発生する
- **THEN** AgentRunner に渡されるプロンプトに以下が含まれる:
  - "This project uses Git for version control, not jj."
  - コンフリクトファイルのリスト
  - Git コンフリクトマーカーの説明（`<<<<<<<`, `=======`, `>>>>>>>`）
  - `git status` の出力
  - 解決後の手順

#### Scenario: Resolution success

- **WHEN** AgentRunner がコンフリクトを解決する
- **AND** `git diff --name-only --diff-filter=U` が空を返す
- **THEN** マージが成功したと判断される
- **AND** 次のブランチのマージに進む

#### Scenario: Resolution failure after retries

- **WHEN** 最大リトライ回数（デフォルト3回）を超えてもコンフリクトが解決されない
- **THEN** エラーメッセージが表示される
- **AND** ワークスペースは保持される（手動検査用）
