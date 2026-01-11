## ADDED Requirements

### Requirement: VCS Backend Selection Flag

CLI は `--vcs` フラグで VCS バックエンドを明示的に選択できなければならない（SHALL）。

#### Scenario: Default auto detection

- **WHEN** `--parallel` フラグが指定される
- **AND** `--vcs` フラグが指定されない
- **THEN** VCS バックエンドが自動検出される（jj 優先）

#### Scenario: Explicit jj selection

- **WHEN** `openspec-orchestrator run --parallel --vcs jj` が実行される
- **THEN** jj バックエンドが使用される
- **AND** jj が利用不可の場合はエラーが表示される

#### Scenario: Explicit git selection

- **WHEN** `openspec-orchestrator run --parallel --vcs git` が実行される
- **THEN** Git バックエンドが使用される
- **AND** jj が存在しても Git が使用される
- **AND** Git が利用不可の場合はエラーが表示される

#### Scenario: Explicit auto selection

- **WHEN** `openspec-orchestrator run --parallel --vcs auto` が実行される
- **THEN** VCS バックエンドが自動検出される
- **AND** jj が優先され、なければ Git が使用される

#### Scenario: Invalid VCS value

- **WHEN** `openspec-orchestrator run --parallel --vcs invalid` が実行される
- **THEN** エラーメッセージ "Invalid VCS backend: invalid. Valid options: auto, jj, git" が表示される
- **AND** 終了コードは非ゼロである

#### Scenario: --vcs without --parallel

- **WHEN** `openspec-orchestrator run --vcs git` が実行される
- **AND** `--parallel` フラグが指定されない
- **THEN** `--vcs` オプションは無視される
- **AND** 通常の逐次実行が行われる

### Requirement: Git Uncommitted Changes Error Message

Git バックエンドで未コミット変更がある場合、CLI は詳細なエラーメッセージを表示しなければならない（SHALL）。

#### Scenario: Error message format

- **WHEN** Git バックエンドで並列実行が試行される
- **AND** 未コミット変更が存在する
- **THEN** エラーメッセージに以下が含まれる:
  - 問題の説明
  - 解決方法（commit または stash）
  - 具体的なコマンド例

#### Scenario: Untracked files also trigger error

- **WHEN** Git バックエンドで並列実行が試行される
- **AND** 未追跡ファイル（untracked files）のみが存在する
- **THEN** 同様のエラーメッセージが表示される
- **AND** `.gitignore` に含まれるファイルは対象外である
