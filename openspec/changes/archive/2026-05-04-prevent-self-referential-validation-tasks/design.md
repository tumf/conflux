# Design

## Overview

Final validation is a gate, not implementation work. When final OpenSpec validation is modeled as a checkbox task, evidence validation must evaluate the task that asks evidence validation to run. That creates a self-referential blocker and can prevent archive from ever promoting the change.

The fix is to make this pattern impossible to miss:

- detect it structurally in the native validator
- stop proposal authoring guidance from generating it
- make archive errors name it directly
- provide a local command that matches archive readiness semantics

## Detection Rule

The validator should inspect checkbox task lines in `tasks.md`. A checkbox task is self-referential when it both:

1. appears as a task checkbox (`- [ ]` or `- [x]`), and
2. asks for final/same-change OpenSpec validation, for example by containing `cflx openspec validate <current-change-id>` or equivalent final validation wording.

The rule should not ban every mention of validation. These are allowed:

- non-checkbox `## Final Validation` sections
- ordinary task verification commands that validate another artifact or run non-OpenSpec tests
- archive gate notes outside checkbox task lists

## Diagnostic

The diagnostic should be specific, not a generic evidence hint warning.

Recommended message shape:

```text
tasks.md:<line>: final OpenSpec validation must not be a checkbox task.
Move it to a non-checkbox "Final Validation" section because archive validation is the authoritative gate.
```

This is more useful than `Verification note should cite repository-verifiable evidence`, which hides the actual failure mode.

## Archive-Gate Semantics

Today, users can run `cflx openspec validate <id> --strict --evidence warn` and see `Validation passed`, while archive may block on evidence warnings. That mismatch is confusing.

The implementation should make the archive readiness policy reproducible locally through one of two acceptable approaches:

1. Add `cflx openspec validate <id> --archive-gate` as the preferred UX.
2. Or explicitly document and use `cflx openspec validate <id> --strict --evidence error` as the archive-equivalent check.

The first option is clearer, because it names the workflow outcome rather than exposing internal severity policy.

## Proposal Authoring Guidance

`cflx-proposal` guidance should continue to recommend final validation, but not as a checkbox task. The safe pattern is:

```markdown
## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate <change-id> --strict --evidence warn`
```

Because this is not a checkbox, implementation agents do not mark it complete and evidence validation does not recursively evaluate it as implementation evidence.

## Constitution Compliance

This change does not introduce new workflow-control state. It only changes validation and prompt rules derived from repository files:

- `tasks.md`
- proposal metadata
- CLI validation flags
- archive prompt/error text

Archive and acceptance decisions remain repository-verifiable, satisfying truthful completion requirements.

## Rejected Alternatives

### Ignore evidence warnings during archive

Rejected. It would reduce archive quality and violate truthful completion. The correct fix is to prevent the self-referential task, not to weaken the gate.

### Keep the generic evidence warning

Rejected. It technically points at a line, but it does not explain the real root cause or remediation.

### Rely only on skill guidance

Rejected. Skills can drift and agents can write custom tasks. The native validator needs the guard.
