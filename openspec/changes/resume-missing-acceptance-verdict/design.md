# Design: Harness-Neutral Missing-Verdict Continuation

## Decision

Handle `MissingVerdict` as a protocol failure with an internal retry transition, not as canonical `CONTINUE` and not as an immediate terminal command error.

The retry starts a normal acceptance command invocation. Continuity comes from Conflux-owned bounded prompt context, not from a provider conversation or external job handle.

## Context Boundary

The continuation packet reuses existing generic inputs:

- acceptance attempt history;
- previous stdout/stderr tails;
- previous findings and the missing-verdict diagnostic;
- current workspace diff and git-derived evidence; and
- the change ID and cycle metadata.

All captured command output remains explicitly marked untrusted and bounded by existing output-tail limits. The corrective instruction is trusted static Conflux text outside the untrusted payload.

## Retry State

Track only the consecutive missing-verdict count needed by the active orchestration run. The state is separate from canonical `CONTINUE` accounting.

```text
acceptance invocation
  canonical verdict: reset protocol count, use existing route
  command failure: use existing terminal command-failure route
  MissingVerdict with count < 2: increment, inject continuation context, invoke acceptance
  MissingVerdict with count = 2: emit exhausted diagnostic, use terminal protocol-failure route
```

The initial invocation plus two retries gives three opportunities to satisfy the verdict protocol. No backoff is required because the next agent invocation is explicitly responsible for checking the current verification state rather than assuming an external notification will arrive.

## Constitutional Restart Behavior

The protocol counter and prior output are not new durable workflow-control state. If Conflux restarts, it evaluates the workspace file/git state. An applied but unarchived change runs acceptance again as a fresh active-run sequence. It cannot archive without a new canonical PASS.

This deliberately trades exact process-local retry-count continuity for compliance with workspace-local workflow state. Logs and runtime history may remain observability, but deletion of out-of-worktree state cannot change the workspace's next authoritative action.

## Mode Parity

Serial and parallel paths should share the retry decision and prompt-context representation. Their process spawning and event plumbing may remain separate, but equivalent observations must produce equivalent routing.

## Rejected Alternatives

### Harness session resume

Rejected because OpenCode, Claude Code, Codex, and other runners expose different or absent session semantics. It would make correctness depend on the selected acceptance harness.

### External job polling

Rejected because Conflux would need to parse provider or `agent-exec` output and own a foreign job lifecycle. The acceptance agent already owns verification completion.

### Reclassify as CONTINUE

Rejected because waiting prose is not a verdict. Reclassification would erase a useful protocol diagnostic and contaminate explicit-`CONTINUE` policy.

### Durable retry checkpoint

Rejected because the constitution forbids out-of-worktree durable state from controlling resume and acceptance routing. A workspace artifact created only to remember protocol retries would also become generated control state without implementation evidence.
