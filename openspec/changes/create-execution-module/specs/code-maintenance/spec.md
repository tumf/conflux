# code-maintenance spec delta

## ADDED Requirements

### Requirement: Execution Module Foundation

システムは `src/execution/` モジュールを提供し、serial mode と parallel mode で共通して使用可能な実行コンテキストと結果型を定義しなければならない（SHALL）。

#### Scenario: ExecutionContext の作成

- **GIVEN** 変更 ID とコンフィグが利用可能である
- **WHEN** 実行コンテキストを作成する
- **THEN** `ExecutionContext` 構造体が作成される
- **AND** workspace_path は serial mode では None、parallel mode では Some(path)

#### Scenario: ExecutionResult の状態遷移

- **GIVEN** 実行処理が開始された
- **WHEN** 処理が完了する
- **THEN** `ExecutionResult::Success`, `ExecutionResult::Failed`, または `ExecutionResult::Cancelled` のいずれかが返される

### Requirement: Progress Information Tracking

システムは実行の進捗情報（完了タスク数、総タスク数、完了率）を追跡するための共通型を提供しなければならない（SHALL）。

#### Scenario: ProgressInfo の計算

- **GIVEN** completed = 3, total = 10 の進捗情報がある
- **WHEN** 完了率を計算する
- **THEN** 30% が返される

#### Scenario: ゼロ除算の回避

- **GIVEN** completed = 0, total = 0 の進捗情報がある
- **WHEN** 完了率を計算する
- **THEN** 0% が返される（ゼロ除算エラーなし）
