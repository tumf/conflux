## MODIFIED Requirements
### Requirement: Shared Parallel Orchestration Service

システムはCLI/TUI双方の並列実行を処理する統合的な`ParallelRunService`を提供しなければならない（SHALL）。

このサービスはイベント通知のためのコールバック機構を受け取り、UI実装が適切にイベントを処理できるようにすること（SHALL）。

このサービスは以下を内包する（SHALL）。
- Git利用可否のチェック
- 依存関係に基づく変更のグルーピング
- ParallelExecutorの調整
- 完了した変更のアーカイブ
- resume時にアーカイブ済みworkspaceを検出するための状態確認

#### Scenario: CLI uses ParallelRunService

- **WHEN** CLIが`--parallel`フラグで実行される
- **THEN** CLIは`ParallelRunService`を用いて変更を実行する
- **AND** イベントはコールバック機構を通じて標準出力にログ出力される

#### Scenario: TUI uses ParallelRunService

- **WHEN** TUIが並列モードで実行される
- **THEN** TUIは`ParallelRunService`を用いて変更を実行する
- **AND** イベントはコールバック機構を通じてTUIイベントチャネルへ送られる

#### Scenario: Parallel mode requires git repository

- **WHEN** 並列実行が要求される
- **AND** `.git`ディレクトリが存在しない
- **THEN** `ParallelRunService`はGitリポジトリが必要である旨のエラーを返す
- **AND** 並列実行は開始されない

#### Scenario: アーカイブ済みworkspaceのresume

- **GIVEN** resume対象workspaceに対応するchangeが`openspec/changes/archive/`へ移動済みである
- **AND** `openspec/changes/{change_id}`が存在しない
- **WHEN** `ParallelRunService`がworkspaceを再利用して実行を開始する
- **THEN** apply/archiveは再実行されない
- **AND** workspaceの現在のrevisionがmerge対象として扱われる
