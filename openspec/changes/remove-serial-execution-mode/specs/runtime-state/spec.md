## MODIFIED Requirements

### Requirement: Additive Runtime Model Introduction

The initial three-level runtime state change remains an additive migration milestone. This follow-up SHALL remove obsolete serial compatibility from execution consumers and SHALL make archive-to-post-archive handling independent of an execution-mode enum.

#### Scenario: follow-up migrates execution consumers

**Given**: the three-level runtime model and reducer coverage exist
**When**: executable CLI, TUI, server control, scheduler, and event consumers are migrated
**Then**: they use cumulative worktree orchestration without serial compatibility adapters
**And**: reducer and snapshot tests cover the sole runtime path

#### Scenario: archive terminal behavior is mode-free

**Given**: a managed-worktree change reaches archived state
**When**: runtime state chooses the next action
**Then**: it enters the configured merge, resolve, or push handling
**And**: no `ExecutionMode::Serial` branch treats archive as terminal
