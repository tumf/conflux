---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - src/acceptance.rs
  - src/agent/prompt.rs
  - src/agent/runner.rs
  - src/orchestration/acceptance.rs
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/serial_run_service.rs
verifications:
  - id: missing-verdict-continuation-local
    requirement: Serial and parallel acceptance continue from bounded Conflux-managed prior output after a missing verdict without harness-specific resume or job APIs
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: focused acceptance orchestration test output and repository quality-gate results recorded in the acceptance review
    rerun: cargo test missing_verdict
    prerequisites: []
---

# Resume Missing Acceptance Verdict

**Change Type**: implementation

## Problem / Context

An acceptance command can finish after reporting that owned verification is still running without emitting a canonical verdict. Conflux correctly classifies this as `MissingVerdict`, distinct from an explicit canonical `CONTINUE`, but serial and parallel orchestration currently route it to a terminal command error. The queue then defers the change as `terminal_error_retry_required` instead of giving acceptance another invocation that knows what the prior attempt was doing.

Conflux already records bounded acceptance stdout/stderr tails and acceptance-attempt history and injects them into later acceptance prompts. That harness-neutral context path can support semantic continuation without OpenCode, Claude Code, or another harness session-resume feature.

The constitution requires workflow control to remain derivable from workspace file/git state. Therefore ordinary missing-verdict retry state remains active-run memory only. A process restart may begin a fresh acceptance sequence for the still-unarchived workspace, but it must run acceptance again and must never infer PASS from prior narrative output.

## Proposed Solution

Keep `MissingVerdict` as a protocol-failure classification and preserve its bounded evidence and diagnostics. Change its orchestration routing from immediate terminal error to a dedicated, bounded acceptance protocol retry in both serial and parallel modes.

For each retry, invoke the configured acceptance command through the existing generic command path. Build the prompt from Conflux-managed information already available across harnesses:

- change ID and current acceptance/apply cycle context;
- bounded stdout and stderr tails from the immediately preceding acceptance attempt;
- recorded attempt findings, including the missing-verdict diagnostic;
- current workspace diff/history context; and
- an explicit corrective instruction to continue the prior investigation, finish or re-check reported verification, and emit exactly one canonical verdict.

Do not invoke harness-specific session identifiers, `resume`/`continue` CLI options, provider event APIs, or external managed-job polling. The acceptance agent remains responsible for waiting for or re-checking work it owns.

Use a dedicated consecutive missing-verdict counter, separate from the explicit-`CONTINUE` counter. Permit at most two protocol retries after the initial attempt. Any canonical verdict resets the consecutive protocol counter and retains its existing routing. Exhaustion becomes the existing terminal missing-verdict error with attempt evidence. A command-launch failure remains a command failure and is not converted into a protocol retry.

Keep this as one proposal because prompt context, retry classification, serial/parallel routing, and queue outcome must change atomically; partial delivery would either retain the terminal failure or retry without sufficient corrective context.

## Acceptance Criteria

- A completed acceptance command with no canonical verdict remains classified as `MissingVerdict`, never as explicit `CONTINUE` or PASS.
- When the active run has protocol-retry budget remaining, serial and parallel orchestration invoke acceptance again instead of returning terminal error or `terminal_error_retry_required`.
- The retry uses the normal configured acceptance command and contains bounded Conflux-managed prior stdout/stderr, attempt evidence, workspace context, and a corrective canonical-verdict instruction.
- The retry path does not use or inspect harness session IDs, harness resume flags, provider-specific events, or external managed-job IDs.
- Missing-verdict retries have a separate consecutive counter and do not consume or alter the configured explicit-`CONTINUE` budget.
- PASS, FAIL, explicit CONTINUE, GATED, permission-stalled, cancellation, and command-failure routing remain unchanged.
- At most two missing-verdict retries occur after one initial attempt; a third consecutive missing verdict becomes the existing terminal protocol failure with bounded evidence and an exhausted-attempt diagnostic.
- A canonical verdict resets the missing-verdict retry sequence before any later acceptance cycle.
- During an available protocol retry, the change remains acceptance work in progress and queue reconciliation does not classify it as `terminal_error_retry_required`.
- No out-of-worktree durable retry checkpoint or acceptance report is introduced. After process restart, an unarchived workspace runs acceptance again from workspace-derived context and cannot be treated as accepted from prior narrative output.

## Explicit Completion Conditions

- Shared retry-decision logic represents `MissingVerdict` continuation separately from canonical `CONTINUE` and is used by both serial and parallel orchestration paths.
- The existing acceptance context builders produce a bounded, untrusted-data-safe continuation prompt that includes corrective instructions only for a missing-verdict retry.
- Serial orchestration no longer maps the first two consecutive `MissingVerdict` results directly to `AcceptanceCommandFailed`.
- Parallel dispatch no longer returns a terminal `WorkspaceResult` while missing-verdict retry budget remains.
- Queue events and logs identify protocol retry attempt/max as non-terminal progress; terminal error wording appears only on launch failure or exhaustion.
- Unit and integration regressions prove retry, context injection, budget separation, canonical reset, exhaustion, queue behavior, and serial/parallel parity without invoking a real AI harness.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cflx openspec validate resume-missing-acceptance-verdict --strict --evidence warn` pass.

## Out of Scope

- Treating status-only output as a canonical verdict.
- Changing canonical verdict syntax or parser semantics.
- Resuming OpenCode, Claude Code, Codex, or other provider sessions.
- Parsing or polling `agent-exec`, provider, or harness job identifiers.
- Persisting retry counters or prior output outside the workspace.
- Writing `ACCEPTANCE_REPORT.json` or another generated workflow checkpoint.
- Guaranteeing survival of an external background process after its owning acceptance command exits.
