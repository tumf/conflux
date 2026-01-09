# Orchestrator

オーケストレーターの実行動作に関する仕様。

## ADDED Requirements

### Requirement: Change Snapshot at Run Start

オーケストレーターは `run` コマンド開始時に存在するchange一覧のスナップショットを取得し、実行中はスナップショット内のchangeのみを処理対象としなければならない（SHALL）。

実行中にユーザーが新しいproposalを追加した場合、そのchangeは次回の `run` コマンドまで処理されない。これにより、中途半端な状態のchangeが実装されることを防ぐ。

#### Scenario: 実行開始後に追加されたchangeは無視される

- **WHEN** オーケストレーターが `run` を開始し、change A, B が存在する
- **AND** 実行中にユーザーが新しい change C を作成する
- **THEN** change C は処理対象から除外される
- **AND** change A, B のみが処理される

#### Scenario: スナップショット内のchangeの進捗は更新される

- **WHEN** オーケストレーターが `run` を開始し、change A が存在する
- **AND** change A のタスクが実行により完了する
- **THEN** change A の進捗は正しく更新される
- **AND** change A が完了したらアーカイブ処理が行われる

#### Scenario: 新規changeが検出された場合のログ出力

- **WHEN** オーケストレーターが実行中に新しいchangeを検出した場合
- **THEN** そのchangeが無視されることをログに出力する
