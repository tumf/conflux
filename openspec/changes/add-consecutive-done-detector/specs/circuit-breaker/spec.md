# Circuit Breaker Capability

## ADDED Requirements

### Requirement: 連続完了シグナル検出

Orchestratorはエージェントの完了シグナルを連続で検出した場合、changeを完了として扱わなければならない（SHALL）。

#### Scenario: 2回連続で完了シグナルが出た場合、changeを完了とマーク
- **GIVEN** あるchangeが2回連続でapplyされている
- **AND** 1回目のapply出力に"Task completed successfully"が含まれる
- **AND** 2回目のapply出力に"All tasks done"が含まれる
- **WHEN** 連続完了シグナル検出を実行する
- **THEN** 2回連続と判定される
- **AND** changeが強制的に完了状態にマークされる
- **AND** 次のループでarchive処理が実行される

#### Scenario: 1回だけでは完了と判定されない
- **GIVEN** あるchangeが1回applyされている
- **AND** apply出力に"Task completed"が含まれる
- **WHEN** 連続完了シグナル検出を実行する
- **THEN** まだ1回のため検出されない
- **AND** 通常通り次のループが実行される

#### Scenario: 完了シグナルが途切れた場合はカウントリセット
- **GIVEN** 1回目のapplyで"done"シグナルが出た
- **AND** 2回目のapplyで完了シグナルが出なかった
- **AND** 3回目のapplyで"completed"シグナルが出た
- **WHEN** 連続完了シグナル検出を実行する
- **THEN** 連続カウントが1にリセットされている
- **AND** まだ完了とは判定されない

#### Scenario: 設定で完了シグナルしきい値を変更できる
- **GIVEN** config内で`done_signal_detector.threshold = 3`が設定されている
- **WHEN** 3回連続で完了シグナルが検出される
- **THEN** changeが完了と判定される

#### Scenario: tasks.mdベースの完了判定と併用される
- **GIVEN** あるchangeのcompleted_tasks == total_tasks
- **OR** 2回連続で完了シグナルが検出された
- **WHEN** orchestratorが完了チェックを実行する
- **THEN** どちらかの条件を満たせば完了と判定される
- **AND** archive処理が実行される
