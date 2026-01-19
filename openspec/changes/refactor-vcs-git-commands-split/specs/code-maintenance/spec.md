## MODIFIED Requirements
### Requirement: VCS Abstraction Layer
システムは VCS バックエンド（Git, Jujutsu）を統一されたトレイトベースの抽象化で管理しなければならない (SHALL)。
各 VCS 実装は専用サブモジュール (`src/vcs/jj/`, `src/vcs/git/`) に配置しなければならない (MUST)。
共通ロジックは `src/vcs/commands.rs` に集約すること。

#### Scenario: 新しい VCS バックエンドを追加する場合
- **WHEN** 開発者が新しい VCS バックエンドを追加する
- **THEN** `src/vcs/<backend>/` にモジュールを作成し、`WorkspaceManager` トレイトを実装するだけで統合可能

#### Scenario: VCS コマンド実行エラー
- **WHEN** VCS コマンドが失敗する
- **THEN** システムは `VcsError` 型で統一されたエラーを返す
- **AND** エラーにはバックエンド種別と詳細メッセージが含まれる

#### Scenario: Git コマンドの責務分割
- **WHEN** 開発者が Git コマンド実装を調査する
- **THEN** commands モジュールが責務別（basic/commit/worktree/merge）に分割されている
