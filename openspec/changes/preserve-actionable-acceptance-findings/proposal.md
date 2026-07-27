---
change_type: implementation
priority: high
dependencies:
  - replace-acceptance-marker-stalls
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/changes/archive/2026-07-21-bound-acceptance-retry-cycles
  - openspec/changes/archive/2026-07-21-harden-acceptance-finding-reconciliation
  - src/acceptance.rs
  - src/history.rs
  - src/orchestration/acceptance.rs
  - src/agent/prompt.rs
  - src/agent/runner.rs
  - src/parallel/executor.rs
  - src/parallel/dispatch.rs
  - src/serial_run_service.rs
  - src/task_parser.rs
verifications:
  - id: actionable-finding-contract-tests
    requirement: Structured and legacy Acceptance findings remain actionable through Apply handoff
    phase: pre-integration
    owner: conflux-acceptance
    trigger: acceptance-review
    automation: Cargo.toml
    evidence: cargo test acceptance && cargo test agent::prompt && cargo test history
    rerun: cargo test acceptance && cargo test agent::prompt && cargo test history
    prerequisites: []
  - id: targeted-repair-loop-tests
    requirement: Serial and parallel repair retries validate finding-to-diff coverage and stop repeated finding IDs
    phase: pre-integration
    owner: conflux-acceptance
    trigger: acceptance-review
    automation: Cargo.toml
    evidence: cargo test orchestration::acceptance && cargo test parallel::dispatch && cargo test serial_run_service
    rerun: cargo test orchestration::acceptance && cargo test parallel::dispatch && cargo test serial_run_service
    prerequisites:
      - replace-acceptance-marker-stalls
---

# Change: Preserve actionable Acceptance findings

**Change Type**: implementation

## Problem/Context

Acceptance can produce a detailed repository-fixable finding, but the current retry path stores the human-readable finding and its normalized comparison identity in the same history slot. `set_checkpoint()` can therefore replace the actionable finding with a lossy value such as `repository|path|verification` before the next Apply prompt is built. Apply receives a file and rule hint without the evidence, required change, or verification expected by Acceptance.

The existing repeated-finding safeguard compares normalized identity sets and broad repository semantic fingerprints. Unrelated source, test, spec, or comment changes can count as progress, and fallback identity may drift when reviewer prose cites a different representative path. This permits repeated off-target Apply work even though the same underlying defect remains open.

The workflow already requires latest-only Acceptance context, stable reconciliation identity, serial/parallel parity, runtime-owned follow-up, and Acceptance-owned completion. This change strengthens those contracts without replaying all attempts, creating a PASS checkpoint, or treating Apply claims as acceptance evidence.

## Proposed Solution

Introduce a JSON-primary structured repository finding with stable `id`, `severity`, `summary`, concrete `evidence`, `required_changes`, and `verification`. Preserve legacy string findings as compatibility input by adapting them to an internal actionable representation without changing the accepted legacy verdict syntax.

Separate the complete latest finding payload from retry identities and semantic fingerprints in history and retry state. Persist enough immutable finding detail in the runtime-owned current follow-up for an interrupted FAIL-to-Apply handoff, while ordinary retry counters and semantic baselines remain in memory. Apply prompts receive the complete latest payload exactly once and enter a focused repair mode in which the current findings outrank completed proposal tasks and historical context.

Before rerunning Acceptance, compare the Apply revision delta with every structured finding's required implementation and verification files. Missing coverage enters an evidenced `acceptance_remediation_mismatch` hold instead of spending another Acceptance invocation. Additional changed files are reported as diagnostics and must have an explicit finding relationship, but line-count limits are not used.

Give each finding ID one automatic repair opportunity. If the next Acceptance FAIL reports the same ID as still open, stop before another Apply with an evidenced `repeated_acceptance_finding` hold, regardless of unrelated semantic progress. A genuinely new finding ID receives its own repair opportunity. Apply may attach remediation evidence, but only a later Acceptance result may close a finding.

## Split Decision

This remains one proposal because finding parsing, full-payload propagation, focused Apply prompting, remediation-diff validation, and per-ID retry stopping share one contract. Shipping any subset independently would preserve a path where actionable detail is lost or unrelated changes still authorize another repair cycle.

## Acceptance Criteria

- JSON FAIL verdicts can carry structured repository findings with stable IDs, severity, evidence, required implementation changes, and required verification changes; legacy string findings continue to parse.
- The complete latest finding payload reaches the next Apply prompt exactly once and is never replaced by its retry identity.
- A restart between FAIL and Apply retains actionable current finding detail in workspace-local follow-up evidence or reruns Acceptance before Apply; missing out-of-worktree state never implies PASS.
- Apply retry guidance limits work to latest open findings, requires per-finding remediation evidence, and does not treat completed proposal tasks as new repair candidates.
- Runtime checks each finding's declared implementation and verification file coverage against the actual post-FAIL diff before invoking Acceptance.
- Missing required diff coverage stops with `acceptance_remediation_mismatch`; unrelated changes and comment-only progress cannot satisfy the finding contract.
- When the next Acceptance FAIL reports the same finding ID, serial and parallel execution stop before a second automatic repair Apply with `repeated_acceptance_finding`, even if other files changed.
- Finding completion remains Acceptance-owned: Apply evidence is a remediation claim, not closure or PASS.
- Operator diagnostics retain the complete open finding, occurrence count, Apply revision, changed files, required-file coverage, unrelated files, remediation evidence, stop reason, and explicit retry next action.

## Explicit Completion Conditions

- `AcceptanceResult`, history, prompt construction, serial execution, and parallel execution use one shared structured finding representation while retaining tested legacy string compatibility.
- Full actionable payload and comparison identity occupy distinct fields and tests prove checkpoint/retry updates cannot overwrite the payload.
- Runtime-owned follow-up rendering and parsing preserve stable ID plus actionable details without allowing Apply to author a closed status.
- Shared repair-loop code validates diff coverage and per-ID retry budget identically in serial and parallel modes.
- Focused tests reproduce the regression shape: a detailed secret-value verification finding cannot be reduced to a path-only prompt, a calibration-only test change fails remediation coverage, and the same ID on the next FAIL starts no additional Apply.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and the default `cargo test` suite pass; any test exceeding one second follows the repository heavy-test convention.

## Dependencies

`replace-acceptance-marker-stalls` must land first because this change reuses its validated stalled lifecycle and revision-bound temporary hold mechanism. This change does not restore Acceptance-origin worktree markers or introduce durable PASS evidence.

## Out of Scope

- Replaying the full Acceptance attempt history.
- Configuring the ten-cycle global safety ceiling.
- Assigning different blocking semantics to `major` and `minor`; both remain FAIL findings.
- Proving semantic correctness from file presence alone; Acceptance remains responsible for semantic review.
- Enforcing an arbitrary changed-line ceiling.
- Redesigning external blocker categories or Apply-origin blocker persistence.
