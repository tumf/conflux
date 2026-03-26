# Change: Add spec-only change mode

## Why
The current Conflux proposal and review workflow assumes that every change primarily drives implementation work. That assumption is too narrow for repository changes whose main output is a canonical spec update. Session analysis showed that spec-only changes are already authorable in practice, but the workflow does not classify them explicitly, does not require archive expectations, and does not give acceptance a first-class way to judge whether the change can be promoted safely.

Making spec-only work explicit reduces ambiguity for proposal authors, helps acceptance avoid demanding nonexistent runtime evidence, and creates a clean place to attach archive-risk warnings for `MODIFIED` / `REMOVED` heavy changes.

## What Changes
- Add explicit `Change Type` classification for `spec-only`, `implementation`, and `hybrid` proposals.
- Teach proposal authoring to use `Specification Tasks` for spec-only changes and to record the expected canonical result of each spec delta.
- Add risk warnings when a spec-only proposal depends on `MODIFIED` / `REMOVED` deltas that can become archive no-ops.
- Teach acceptance guidance to evaluate archive-readiness for spec-only changes instead of requiring unrelated runtime integration evidence.
- Add regression coverage for proposal validation and acceptance behavior across spec-only, implementation, and hybrid proposals.

## Impact
- Affected specs: `spec-only-changes`, `agent-prompts`
- Affected code: `skills/cflx-proposal/SKILL.md`, `skills/cflx-proposal/scripts/cflx.py`, `skills/cflx-workflow/references/cflx-accept.md`, `skills/cflx-workflow/SKILL.md`
- Dependencies: builds on the archive-simulation mechanics proposed in `update-spec-archive-promotion` for the strongest spec-only safety checks

## Non-Goals
- Replacing implementation-oriented proposals as the default Conflux workflow
- Relaxing evidence requirements for runtime-bearing changes
- Bundling post-run reporting changes into the same proposal

## Success Criteria
- Proposal tooling can distinguish `spec-only`, `implementation`, and `hybrid` changes without asking the user to invent a custom workflow.
- Spec-only proposals use `Specification Tasks` and capture the expected canonical result for each spec delta.
- Proposal tooling emits an archive-risk warning when a spec-only proposal relies on risky `MODIFIED` / `REMOVED` deltas.
- Acceptance guidance evaluates spec-only archive readiness explicitly and fails when promotion would be a no-op.
