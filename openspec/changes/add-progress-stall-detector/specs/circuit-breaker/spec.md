# Circuit Breaker Capability

## ADDED Requirements

### Requirement: 進捗停滞検出

Orchestratorはchangeのタスク進捗が停滞した場合に検出し、無限ループを防止しなければならない（SHALL）。

#### Scenario: 3回連続で進捗なしの場合、changeをスキップ
- **GIVEN** あるchangeが3回連続でapplyされている
- **AND** 各apply実行後にcompleted_tasksが変化していない
- **WHEN** orchestratorが4回目のapplyを試みようとする
- **THEN** 進捗停滞を検出してwarningログを出力する
- **AND** 次のchangeへ自動的に移行する

#### Scenario: 正常に進捗する場合は検出されない
- **GIVEN** あるchangeが2回applyされている
- **AND** 2回目のapply後にcompleted_tasksが増加した
- **WHEN** orchestratorが3回目のapplyを実行する
- **THEN** 進捗停滞は検出されない
- **AND** 通常通りapplyが実行される

#### Scenario: 設定で進捗停滞検出を無効化できる
- **GIVEN** config内で`stall_detection.enabled = false`が設定されている
- **WHEN** changeが10回連続で進捗なしでapplyされている
- **THEN** 進捗停滞検出は行われない
- **AND** max_iterationsまでループが継続される
