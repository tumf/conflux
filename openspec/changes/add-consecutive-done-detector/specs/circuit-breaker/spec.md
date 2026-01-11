# Circuit Breaker Capability

## ADDED Requirements

### Requirement: Consecutive done signal detection

The Orchestrator MUST mark a change as complete when the agent returns consecutive "done" signals.

#### Scenario: 2回連続で完了シグナルが出た場合、changeを完了とマーク

**Given** あるchangeが2回連続でapplyされている  
**And** 1回目のapply出力に"Task completed successfully"が含まれる  
**And** 2回目のapply出力に"All tasks done"が含まれる  
**When** 連続完了シグナル検出を実行する  
**Then** 2回連続と判定される  
**And** changeが強制的に完了状態にマークされる  
**And** 次のループでarchive処理が実行される

#### Scenario: 1回だけでは完了と判定されない

**Given** あるchangeが1回applyされている  
**And** apply出力に"Task completed"が含まれる  
**When** 連続完了シグナル検出を実行する  
**Then** まだ1回のため検出されない  
**And** 通常通り次のループが実行される

#### Scenario: 完了シグナルが途切れた場合はカウントリセット

**Given** 1回目のapplyで"done"シグナルが出た  
**And** 2回目のapplyで完了シグナルが出なかった  
**And** 3回目のapplyで"completed"シグナルが出た  
**When** 連続完了シグナル検出を実行する  
**Then** 連続カウントが1にリセットされている  
**And** まだ完了とは判定されない

#### Scenario: 設定で完了シグナルしきい値を変更できる

**Given** config内で`done_signal_detector.threshold = 3`が設定されている  
**When** 3回連続で完了シグナルが検出される  
**Then** changeが完了と判定される

#### Scenario: tasks.mdベースの完了判定と併用される

**Given** あるchangeのcompleted_tasks == total_tasks  
**Or** 2回連続で完了シグナルが検出された  
**When** orchestratorが完了チェックを実行する  
**Then** どちらかの条件を満たせば完了と判定される  
**And** archive処理が実行される
