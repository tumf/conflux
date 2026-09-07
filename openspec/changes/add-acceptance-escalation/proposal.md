---
change_type: implementation
priority: high
dependencies: []
references:
  - src/config/types.rs
  - src/agent/runner.rs
  - src/orchestration/acceptance.rs
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - openspec/specs/configuration/spec.md
  - openspec/specs/parallel-execution/spec.md
verifications:
  - id: acceptance-escalation-tests
    requirement: Invalid Acceptance outcomes use the optional escalation command once without escalating semantic FAIL or changing workspace-derived routing
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/acceptance.rs
    evidence: cargo test acceptance_escalation --lib
    rerun: cargo test acceptance_escalation --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add bounded Acceptance escalation

**Change Type**: implementation

## Problem / Context

Conflux already retries Acceptance command failures and verdict-protocol failures with bounded, Conflux-managed context. Every retry still uses the same configured `acceptance_command`. If that reviewer repeatedly emits no canonical verdict, a bare blocker, a malformed structured finding, or the runtime's generic empty-FAIL fallback, retrying the same reviewer can exhaust the protocol budget without obtaining an actionable finding.

`apply_escalation_command` cannot help this path. It is selected only for late consecutive empty-WIP Apply retries. A valid semantic Acceptance FAIL must continue to return to Apply and must not be treated as reviewer failure.

## Proposed Solution

Add an optional top-level `acceptance_escalation_command` and bounded policy under a new `acceptance_escalation` object:

- `after_invalid_results`: number of consecutive invalid Acceptance results before the next Acceptance-only retry uses the escalation command; default `1`.
- `max_uses_per_sequence`: maximum escalation-command uses during one consecutive invalid-result sequence; default `1`.

An invalid result is limited to `MissingVerdict`, `BareBlocker`, `MalformedFinding`, or a FAIL carrying no actionable findings and therefore otherwise reduced to the exact generic fallback `Investigate acceptance failure and apply the required fix`. A canonical PASS, actionable FAIL, CONTINUE, validated external blocker, permission hold, runtime limit, cancellation, and transport-level command failure are not invalid-result escalation triggers.

When eligible, the next Acceptance retry runs `acceptance_escalation_command` with the same `{change_id}` and generated `{prompt}` contract as `acceptance_command`. The prompt retains the existing bounded corrective context and states that the alternate reviewer must produce one fresh canonical verdict from the current repository evidence. If the optional command is absent, existing normal-command retry behavior remains unchanged.

Counters and command selection remain active-run memory only. Any non-invalid completed result resets the invalid-result sequence. Restart recomputes routing from workspace and Git evidence as required by the constitution.

## Acceptance Criteria

- Configuration loads and merges the optional escalation command and validates both positive policy values.
- The first configured invalid result schedules an Acceptance-only retry through the escalation command when default policy and command are present.
- Escalation use is capped once per invalid-result sequence by default; exhaustion follows existing typed protocol/failure routing rather than creating an unbounded loop.
- CLI/TUI/remote managed-worktree execution share one eligibility and accounting policy.
- Semantic FAIL with actionable findings always follows FAIL-to-Apply routing and never selects Acceptance escalation.
- Missing optional configuration preserves current behavior without warning or load failure.
- No escalation counter, checkpoint, or routing marker is persisted outside the worktree.

## Explicit Completion Conditions

- `OrchestratorConfig` exposes merged getters and validation for `acceptance_escalation_command` and `acceptance_escalation`.
- `AgentRunner` can run either Acceptance template through one prompt-construction path, preserving placeholders, skill guidance, bounded history, protocol retry, and command-recovery context.
- Shared Acceptance orchestration owns invalid-result classification, counter reset, eligibility, cap, and operator-visible diagnostics.
- Serial and parallel execution select the same typed command mode and do not duplicate policy decisions.
- Focused tests cover config merging/validation, each invalid class, the exact empty-FAIL fallback, semantic FAIL exclusion, reset behavior, missing-command fallback, command selection, and use-cap exhaustion.

## Out of Scope

- Escalating a valid semantic FAIL merely because the same defect reappears.
- Changing Apply escalation, Apply stall thresholds, external-blocker classification, or Acceptance runtime limits.
- Persisting retry counters or alternate-reviewer state outside the workspace.
- Adding provider-specific session, resume, model, or command semantics.
- Automatically repairing findings during Acceptance.
