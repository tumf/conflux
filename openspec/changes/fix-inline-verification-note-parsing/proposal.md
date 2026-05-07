---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd.rs
  - openspec/specs/cflx-proposal-validation/spec.md
  - openspec/CONSTITUTION.md
---

# Fix inline verification note parsing

**Change Type**: implementation

## Problem / Context

Runtime logs since `.last-checked` show repeated archive-gate failures where tasks containing inline notes such as `(verification: manual - ...)` were still reported as `Behavior-bearing task missing '(verification: ...)' note`. The current validator regex in `src/openspec_cmd.rs` only recognizes inline verification notes when they appear at the very end of a checkbox line, optionally followed by punctuation. In real Conflux tasks, agents often append completion-condition text after the verification note, which makes the validator miss the note and produce a misleading archive-blocking error.

This conflicts with the constitution's truthful completion principle because agents are forced into repeated checklist wording edits even when repository-verifiable evidence exists in the same task line.

## Proposed Solution

Update native OpenSpec task validation so inline `(verification: ...)` notes are detected anywhere in a checkbox task line, not only at the end. Preserve the existing evidence and ownership checks against the captured note text, and add regression coverage for inline verification notes followed by completion-condition text.

## Acceptance Criteria

- `cflx openspec validate <id> --archive-gate` accepts checkbox tasks whose inline `(verification: ownership - evidence)` note is followed by additional completion-condition prose.
- Missing verification notes still fail in `--archive-gate` / `--evidence error` mode.
- Weak verification notes that lack repository-verifiable evidence still produce the existing evidence finding.
- Self-referential final OpenSpec validation checkbox detection remains unchanged.

## Explicit Completion Conditions

- `src/openspec_cmd.rs` has deterministic parsing for inline verification notes that are not line-terminal.
- Unit tests cover the logged failure shape and the existing failure modes.
- Focused validation/tests pass for the OpenSpec validator surface.

## Out of Scope

- Changing acceptance-review quality judgment for task adequacy.
- Relaxing evidence ownership markers or repository-verifiable evidence requirements.
- Using log files as workflow-control state.
