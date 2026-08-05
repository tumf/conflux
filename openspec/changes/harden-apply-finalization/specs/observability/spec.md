## ADDED Requirements

### Requirement: Final commit hook output MUST stream with structured context

Conflux MUST stream final Apply commit stdout and stderr to user-visible TUI output and persistent logs while retaining the complete raw command result required for repository rejection and index-lock classification. Each emitted line MUST identify the change, `commit` operation, source stream, and finalization attempt. Presentation MAY strip ANSI control sequences, but classification buffers MUST preserve raw output. Separate retry attempts MUST remain distinguishable and MUST NOT be silently deduplicated.

#### Scenario: Successful pre-commit progress is visible

**Given**: a hook-enabled final Apply commit writes progress to stdout or stderr and later succeeds
**When**: Conflux executes the commit
**Then**: each progress line is visible in normal TUI output and persistent logs before or at process completion
**And**: each line identifies the change, commit operation, stream, and attempt

#### Scenario: Hook rejection retains full and bounded evidence

**Given**: a final commit hook emits diagnostics and rejects the commit
**When**: Conflux records the failure
**Then**: persistent logs retain the complete streamed output
**And**: the next Apply prompt receives only bounded diagnostic tails under the existing prompt budget
**And**: typed rejection uses the preserved exit status and raw streams

#### Scenario: Index-lock retry output remains attributable

**Given**: final commit encounters eligible managed-worktree index-lock contention
**When**: Conflux retries finalization
**Then**: output from each attempt is labeled with its attempt number
**And**: repeated lines from separate attempts are not removed as duplicates
**And**: the complete raw stderr remains available to the existing lock classifier

#### Scenario: Long-running silent hook remains observable

**Given**: final commit hooks are still running but have emitted no recent output
**When**: the TUI renders commit progress
**Then**: the operator can see that pre-commit remains active under the commit phase
**And**: the presentation does not fabricate hook success or alter workflow-control state
