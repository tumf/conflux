# CLI Spec Delta: Fix Footer Progress Tracking

## MODIFIED Requirements

### Requirement: 実行中フッターの進捗バー表示

実行モードのフッターには、全体の処理進捗をバーで表示しなければならない（SHALL）。

#### Scenario: 実行中の進捗バー表示
- **WHEN** TUIが実行モードである
- **THEN** フッターにキュー内全タスクの進捗バーが表示される
- **AND** 進捗バーは完了タスク数/総タスク数に基づいて計算される
- **AND** パーセンテージが数値で表示される

#### Scenario: 進捗バーの計算方法
- **WHEN** 進捗バーを表示する
- **THEN** 総タスク数は処理対象全変更（Queued, Processing, Completed, Archived）の `total_tasks` の合計である
- **AND** 完了タスク数は処理対象全変更の `completed_tasks` の合計である
- **AND** 進捗率は `completed_tasks / total_tasks * 100` で計算される
- **AND** NotQueued および Error 状態の変更は進捗計算に含まれない

#### Scenario: 完了タスクの進捗保持
- **WHEN** 変更が Completed または Archived 状態に遷移する
- **THEN** その変更のタスク進捗は引き続き進捗バーの計算に含まれる
- **AND** 進捗パーセンテージは減少しない（単調増加）

#### Scenario: タスク数が0の場合
- **WHEN** 進捗バーを表示する
- **AND** 総タスク数が0である
- **THEN** 進捗バーは0%として表示される
