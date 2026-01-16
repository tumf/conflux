# Spec Delta: parallel-execution

## MODIFIED Requirements

### Requirement: VCS Backend Abstraction

システムは並列実行のために VCS バックエンド抽象化レイヤーを提供しなければならない（SHALL）。

`WorkspaceManager` trait の `original_branch()` メソッドは、ベースブランチ名を返さなければならない（SHALL）。ベースブランチが未初期化の場合、`None` を返さなければならない（SHALL）。

システムは `original_branch()` が `None` を返す場合、適切なエラーメッセージとともに処理を中断しなければならない（SHALL）。ベースブランチとして特定の値（"main", "develop", "master" など）をハードコードしてはならない（MUST NOT）。

**変更理由**: ベースブランチを動的に取得する現在の設計を維持しつつ、フォールバックによるハードコードを排除し、明示的なエラーハンドリングを実現するため。

#### Scenario: original_branch returns current branch

- **WHEN** ワークスペースマネージャーが初期化される
- **AND** `get_current_branch()` が正常にブランチ名を返す
- **THEN** `original_branch()` はそのブランチ名を返す
- **AND** 返されるブランチ名は実行時のカレントブランチと一致する

#### Scenario: original_branch returns None before initialization

- **WHEN** ワークスペースマネージャーが作成されたが `create_worktree()` がまだ呼ばれていない
- **AND** `original_branch()` が呼ばれる
- **THEN** `None` を返す

#### Scenario: Error when original_branch is None during merge

- **WHEN** マージ処理が実行される
- **AND** `original_branch()` が `None` を返す
- **THEN** システムはエラーを返す
- **AND** エラーメッセージは "Original branch not initialized" を含む
- **AND** マージ処理は実行されない

### Requirement: Workspace State Detection

システムは既存workspaceの再開時に、正確な状態を検出し、適切なアクションを実行しなければならない（SHALL）。

状態検出は `detect_workspace_state(change_id, workspace_path, base_branch)` 関数により行われる。`base_branch` パラメータは、マージ状態の検出に使用されなければならない（SHALL）。

システムは、マージ状態の検出において特定のブランチ名をハードコードしてはならない（MUST NOT）。呼び出し側から渡された `base_branch` パラメータを使用しなければならない（SHALL）。

**変更理由**: 状態検出関数が任意のベースブランチに対応できるようにし、"main" ブランチへのハードコード依存を除去するため。

#### Scenario: Detect merged state with custom base branch

- **WHEN** `detect_workspace_state()` が呼ばれる
- **AND** `base_branch` パラメータが "develop" である
- **AND** Archive コミットが "develop" ブランチにマージ済みである
- **THEN** 状態は `WorkspaceState::Merged` として判定される

#### Scenario: Detect archived state with custom base branch

- **WHEN** `detect_workspace_state()` が呼ばれる
- **AND** `base_branch` パラメータが "feature/test" である
- **AND** Archive コミットが存在するが "feature/test" ブランチにマージされていない
- **THEN** 状態は `WorkspaceState::Archived` として判定される

#### Scenario: Error when base_branch parameter is missing

- **WHEN** システムが状態検出を実行しようとする
- **AND** `base_branch` パラメータが提供されていない（関数シグネチャの変更前）
- **THEN** コンパイルエラーが発生する
- **AND** 開発者は明示的に `base_branch` を渡すことを要求される

### Requirement: Git Sequential Merge

Git バックエンド使用時、システムは複数ブランチを逐次マージしなければならない（SHALL）。

マージ処理において、ターゲットブランチ（統合先ブランチ）は `original_branch()` から取得しなければならない（SHALL）。`original_branch()` が `None` を返す場合、システムはエラーを返さなければならない（SHALL）。

システムは、マージターゲットとして特定のブランチ名（"main", "develop" など）をハードコードしてはならない（MUST NOT）。

**変更理由**: マージ処理が任意のベースブランチで動作するようにし、フォールバックによる予期しない動作を防ぐため。

#### Scenario: Merge to dynamically determined branch

- **WHEN** Git バックエンドが複数ブランチのマージを実行する
- **AND** `original_branch()` が "develop" を返す
- **THEN** すべてのマージは "develop" ブランチに対して実行される
- **AND** "main" ブランチは参照されない

#### Scenario: Merge fails when original_branch is None

- **WHEN** システムがマージを開始しようとする
- **AND** `original_branch()` が `None` を返す
- **THEN** マージは実行されない
- **AND** エラーメッセージ "Original branch not initialized" が返される
- **AND** ユーザーにワークスペースの再作成を促す

#### Scenario: Merge verification uses original_branch

- **WHEN** システムがマージ後の検証を実行する
- **AND** `original_branch()` が "feature/main-work" を返す
- **THEN** 検証は "feature/main-work" ブランチに対するマージを確認する
- **AND** 他のブランチ（"main" など）は検証されない
