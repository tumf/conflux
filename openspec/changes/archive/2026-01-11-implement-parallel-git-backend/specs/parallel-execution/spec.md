## MODIFIED Requirements

### Requirement: VCS Backend Abstraction

システムは並列実行のために VCS バックエンド抽象化レイヤーを提供しなければならない（SHALL）。

`WorkspaceManager` trait は以下の操作を定義する:
- VCS 利用可能性チェック
- ワークスペース作成
- リビジョン取得
- マージ
- クリーンアップ
- 作業コピーのスナップショット
- コミットメッセージ設定
- ワークスペース内リビジョン取得
- VCS ステータス取得
- コンフリクト検出
- 緊急クリーンアップ（同期版）

#### Scenario: JjWorkspaceManager implements trait

- **WHEN** jj リポジトリで並列実行が開始される
- **THEN** `JjWorkspaceManager` が `WorkspaceManager` trait を実装する
- **AND** 既存の jj ベースの並列実行動作は変更されない

#### Scenario: GitWorkspaceManager implements trait

- **WHEN** Git リポジトリで並列実行が開始される
- **THEN** `GitWorkspaceManager` が `WorkspaceManager` trait を実装する
- **AND** Git Worktree を使用してワークスペースを管理する

#### Scenario: ParallelExecutor uses trait object

- **WHEN** `ParallelExecutor` が初期化される
- **THEN** `workspace_manager` は `Box<dyn WorkspaceManager>` として保持される
- **AND** VCS バックエンドは設定または自動検出により決定される
