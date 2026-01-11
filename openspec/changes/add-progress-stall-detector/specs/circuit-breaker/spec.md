# Circuit Breaker Capability

## ADDED Requirements

### Requirement: Progress stall detection

The Orchestrator MUST detect when a change's task progress has stalled and prevent infinite loops.

#### Scenario: 3回連続で進捗なしの場合、changeをスキップ

**Given** あるchangeが3回連続でapplyされている  
**And** 各apply実行後にcompleted_tasksが変化していない  
**When** orchestratorが4回目のapplyを試みようとする  
**Then** 進捗停滞を検出してwarningログを出力する  
**And** 次のchangeへ自動的に移行する

#### Scenario: 正常に進捗する場合は検出されない

**Given** あるchangeが2回applyされている  
**And** 2回目のapply後にcompleted_tasksが増加した  
**When** orchestratorが3回目のapplyを実行する  
**Then** 進捗停滞は検出されない  
**And** 通常通りapplyが実行される

#### Scenario: 設定で進捗停滞検出を無効化できる

**Given** config内で`stall_detection.enabled = false`が設定されている  
**When** changeが10回連続で進捗なしでapplyされている  
**Then** 進捗停滞検出は行われない  
**And** max_iterationsまでループが継続される
