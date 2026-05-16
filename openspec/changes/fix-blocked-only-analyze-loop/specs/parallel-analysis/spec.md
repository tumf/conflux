## ADDED Requirements

### Requirement: Dependency analysis only runs for dispatchable ordinary work

Parallel dependency analysis SHALL be invoked only when at least one ordinary queued candidate may become dispatchable apply work. The scheduler MUST classify manual waits, reducer-owned lane waiters, terminal-error retry-required changes, dependency-blocked changes, and unavailable candidates before invoking `analyze_command`.

When no ordinary dispatchable candidate exists, the scheduler MUST skip dependency analysis for that iteration and rely on blocked-only drain, scheduler-owned retry dispatch, or notification-driven wake behavior instead.

Analyzer failure diagnostics SHOULD be deduplicated for stable queued/in-flight/error signatures. This dedupe is runtime-only observability state and MUST NOT participate in workflow routing decisions.

<!-- Expected canonical result after archive: `parallel-analysis` will prohibit analyzer invocation for blocked-only/manual-wait-only scheduler states and require stable diagnostic dedupe for repeated analyzer failures. -->

#### Scenario: manual merge-wait rows do not trigger analysis

**Given**: queued scheduler state contains only changes currently represented as manual `MergeWait`
**And**: no reducer-owned resolve/reject retry work is pending
**When**: the scheduler evaluates whether to run dependency analysis
**Then**: it skips `analyze_command`
**And**: the changes remain manual merge-wait rows until explicit `ResolveMerge` intent is accepted

#### Scenario: terminal-error retry-required rows do not trigger analysis

**Given**: queued scheduler state contains only recoverable terminal-error changes that require explicit retry
**When**: the scheduler evaluates whether to run dependency analysis
**Then**: it skips `analyze_command`
**And**: no ordinary apply dispatch is created for those changes

#### Scenario: analyzer failure diagnostic is deduplicated

**Given**: dependency analysis for the same queued IDs and in-flight IDs repeatedly fails with the same normalized analyzer error
**When**: later scheduler iterations observe the unchanged failure signature
**Then**: operator-visible analyzer failure diagnostics are not emitted repeatedly
**And**: a later changed queued, in-flight, or error signature may emit a fresh diagnostic

#### Scenario: ordinary queued work still triggers analysis

**Given**: at least one ordinary queued change is neither terminal, active, manual merge-wait, scheduler-owned lane-wait, dependency-blocked, nor candidate-unavailable
**When**: an execution slot is available
**Then**: the scheduler may invoke dependency analysis for the ordinary dispatchable working set
**And**: normal dependency ordering and dispatch selection continue to apply
