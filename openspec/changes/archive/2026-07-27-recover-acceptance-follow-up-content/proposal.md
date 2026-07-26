---
change_type: implementation
priority: high
dependencies: []
references:
  - src/task_parser.rs
  - src/execution/apply.rs
  - src/serial_run_service.rs
  - src/parallel/dispatch.rs
  - src/openspec_cmd/validation.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/agent-prompts/spec.md
verifications:
  - id: acceptance-follow-up-recovery-tests
    requirement: Unknown content in an unambiguous runtime-owned acceptance follow-up is preserved without stopping workflow execution or corrupting task progress
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: scripts/test-time-top10.sh
    evidence: cargo test output covering recovery, idempotency, task counting, archive validation, serial and parallel routing, and hard-error boundaries
    rerun: cargo test --all-targets
    prerequisites: []
---

# Recover Acceptance Follow-up Content

**Change Type**: implementation

## Problem / Context

Conflux treats acceptance follow-up sections in `tasks.md` as runtime-owned and replaces or removes them during retry hydration and acceptance PASS cleanup. The current safety check rejects the entire update when the section contains any line outside a narrow allowlist. An apply agent can therefore add harmless multiline evidence or vary metadata capitalization and cause the apply workflow to terminate with `Acceptance follow-up contains non-runtime content` even though the section boundary and intended runtime tasks remain clear.

Refusing every unfamiliar line prevents data loss, but it turns recoverable formatting drift into a terminal execution error. Expanding the allowlist alone only postpones the next failure. Silently deleting the content would violate the same safety goal.

## Proposed Solution

Replace the blanket refusal with bounded preserve-and-recover behavior. When runtime can identify an acceptance follow-up section and its boundary unambiguously, it preserves unknown content byte-for-byte in a normal `## Recovered Acceptance Notes` section, renders that content inside a dynamically sized fenced literal, emits a warning, and continues the requested replacement or PASS cleanup. The recovered block is explicitly non-instructional and non-task state.

The recovery and runtime-section update occur in one atomic file replacement and are idempotent across retries and restarts. Existing recovered content is matched by its exact preserved bytes so the same material is not appended repeatedly. Known runtime lines remain runtime-owned and are not copied into recovered notes.

Task progress parsing and OpenSpec task validation become fence-aware so recovered checkbox-like text cannot affect completion or archive gates. Hard errors remain for unreadable files, malformed encoding, unclosed fences, or any layout where runtime cannot determine the destructive-edit boundary safely.

Keep this as one proposal because recovery storage is unsafe without fence-aware task accounting, while parser changes alone do not prevent workflow termination. The preservation, normalization, and routing behavior must ship together.

## Acceptance Criteria

- Harmless unknown text inside an unambiguous runtime-owned acceptance follow-up no longer terminates apply, acceptance retry, or PASS cleanup.
- Unknown content is preserved byte-for-byte under `## Recovered Acceptance Notes` in a dynamically sized fenced literal with a fixed notice that it is not instructions or task state.
- The same unknown content produces at most one recovered block across hydration, retry, PASS cleanup, process restart, and repeated normalization.
- Recovery and follow-up replacement or removal are committed through one atomic tasks-file update; a failed write leaves the original file unchanged.
- Recovered checkbox text, headings, and verification prose do not affect task completion counts, proposal validation, or archive validation.
- Serial and parallel execution apply equivalent recovery, warning, and cleanup behavior.
- Ambiguous or structurally unsafe Markdown remains a hard error and is not modified.
- Acceptance FAIL remains the primary diagnosis when persistence cannot safely complete; recovery diagnostics remain supplemental.

## Explicit Completion Conditions

- `src/task_parser.rs` classifies known runtime lines, extracts unknown byte ranges, selects a fence longer than any backtick run in recovered content, deduplicates recovered blocks, and writes the complete result atomically.
- Task progress and task-validation scanners ignore checkbox syntax inside backtick and tilde fenced blocks, with tests covering dynamic fences and embedded runtime headings.
- Apply hydration, FAIL replacement, and PASS cleanup use the shared recovery path in serial and parallel execution.
- Tests prove successful recovery, exact preservation, idempotency, restart behavior, PASS cleanup retention, write-failure non-destruction, and hard-error handling for ambiguous boundaries and unclosed fences.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets` pass.

## Out of Scope

- Preserving runtime-recognized attempt, finding, evidence, identity, or next-action lines as user notes.
- Executing, interpreting, or trusting recovered content.
- Moving workflow-control state outside the workspace.
- Repairing arbitrary malformed Markdown when the runtime-owned section boundary cannot be established safely.
- Adding retention limits or deleting distinct recovered blocks automatically.
