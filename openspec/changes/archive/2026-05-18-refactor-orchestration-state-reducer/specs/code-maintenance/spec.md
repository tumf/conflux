## ADDED Requirements

### Requirement: Orchestration reducer refactor preserves state transitions

オーケストレーション状態 reducer の責務分割は、既存の command/event 入力に対する terminal state、activity state、wait state、queue intent、resolve/reject wait queue の外部観測可能な結果を変更してはならない。

#### Scenario: reducer command の状態遷移が維持される

**Given**: 既存の `OrchestratorState` と代表的な `ReducerCommand` がある
**When**: reducer の責務分割後に command を適用する
**Then**: `ReduceOutcome`、display status、terminal state、activity state、wait state、queue intent は分割前と同等である
**And**: resolve/reject wait queue の membership は分割前と同等である

#### Scenario: execution event の状態遷移が維持される

**Given**: archive、merge、resolve、rejection review の代表的な `ExecutionEvent` がある
**When**: reducer の責務分割後に event を適用する
**Then**: display status と terminal/activity/wait state は分割前と同等である
**And**: workspace 外の durable state、ログ、UI 状態は workflow-control 入力として使われない
