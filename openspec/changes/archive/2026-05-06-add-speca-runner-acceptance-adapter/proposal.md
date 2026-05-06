---
change_type: implementation
priority: high
dependencies: []
references:
  - skills/cflx-accept-with-speca/SKILL.md
  - .opencode/commands/cflx-accept.md
  - openspec/specs/agent-prompts/spec.md
  - .agents/skills/local/speca-openspec-audit-trial/SKILL.md
---

# Change: Add SPECA runner acceptance adapter guidance

**Change Type**: implementation

## Premise / Context

- `cflx-accept-with-speca` currently adds a SPECA-style property/proof-attempt lens but does not require or document a concrete NyxFoundation/speca runner invocation.
- The prior `add-speca-acceptance-skill` change explicitly left first-class external SPECA runner integration out of scope.
- The existing acceptance contract remains owned by `.opencode/commands/cflx-accept.md`; this change must not introduce another verdict protocol or alter parser behavior.
- `openspec/CONSTITUTION.md` requires workflow-control decisions to be based on workspace-local, repository-verifiable evidence and forbids out-of-worktree durable state as authoritative workflow state.

## Problem

Users selecting `accept_skill = "cflx-accept-with-speca"` reasonably expect the acceptance review to attempt the official NyxFoundation/speca pipeline when it is available, not only a manual SPECA-style review. Today the bundled skill says to use an external runner if installed and usable, but it does not define where the runner lives, how to prepare inputs safely, which command shape to use, how failures are classified, or how to keep official SPECA output from becoming hidden workflow-control state.

## Proposed Solution

Extend the bundled `cflx-accept-with-speca` skill with a concrete optional SPECA runner adapter workflow:

1. Detect whether the official NyxFoundation/speca repository and prerequisites are available outside the Conflux worktree, defaulting to `~/tmp/speca` as the clone/cache location.
2. Prepare SPECA input/output directories outside the Conflux repository so proposal, implementation, and git state remain clean unless a later task intentionally edits tracked files.
3. Run the official runner through `uv run python3 scripts/run_phase.py ...` from the SPECA repo when prerequisites are satisfied, using the phase(s) and arguments documented by the installed SPECA checkout.
4. Capture whether official SPECA outputs were produced, and use those outputs only as supporting evidence for the property/proof-attempt review.
5. Fall back to manual SPECA-style property review when the runner is absent, prerequisites fail, auth is unavailable, or the runner cannot produce usable outputs.
6. Preserve the existing Conflux acceptance verdict contract and map any blocking runner-backed property failure to the standard JSON `fail` findings.

## Acceptance Criteria

1. `skills/cflx-accept-with-speca/SKILL.md` documents an optional official SPECA runner path using NyxFoundation/speca and `uv run python3 scripts/run_phase.py ...` without making the runner mandatory for acceptance.
2. The documented adapter keeps clones, generated inputs, runner outputs, and logs outside the Conflux worktree unless an implementation task explicitly chooses a tracked fixture or test artifact.
3. The skill explains prerequisite checks for `uv`, the SPECA checkout, Python dependencies, and Claude/API/session availability before launching a potentially long runner command.
4. The skill requires long or noisy runner setup/execution to use `agent-exec run -- ...` so acceptance remains observable and context-efficient on mini.
5. Runner outputs are treated as supporting evidence only; they must not become authoritative workflow-control state and must not replace workspace git/file evidence.
6. If official SPECA execution fails or is unavailable, the reviewer must report the limitation in human-readable reasoning and continue with manual SPECA-style property review rather than automatically passing or emitting a special SPECA verdict.
7. The final acceptance output remains the standard Conflux JSON verdict contract from `.opencode/commands/cflx-accept.md`, with no `SPECA: PASS/FAIL` terminal protocol.
8. Unit tests or embedded-skill contract tests fail if the updated skill omits the runner command guidance, omits outside-worktree safety guidance, or introduces a SPECA-specific verdict protocol.

## Explicit Completion Conditions

- `skills/cflx-accept-with-speca/SKILL.md` includes a dedicated official SPECA runner section covering clone/cache location, prerequisite checks, input/output placement, `uv run python3 scripts/run_phase.py ...` invocation guidance, failure fallback, and evidence classification.
- `src/embedded_skills.rs` tests or equivalent unit coverage assert the embedded skill includes the runner guidance and still preserves verdict ownership and no-SPECA-terminal constraints.
- Any command snippets added to the skill avoid hard-coded repository-specific destructive operations and keep target worktree mutations out of the official SPECA runner preparation flow.
- `cargo test embedded_skills` passes.
- `cflx openspec validate add-speca-runner-acceptance-adapter --strict --evidence warn` passes.

## Out of Scope

- Vendoring the NyxFoundation/speca source into Conflux.
- Adding a `cflx speca` CLI subcommand or a Rust wrapper that directly executes SPECA.
- Changing `acceptance_command`, acceptance parser behavior, operation skill configuration semantics, or verdict JSON schema.
- Making official SPECA execution a hard acceptance requirement when prerequisites, auth, or runner support are unavailable.
- Persisting SPECA outputs as durable workflow-control state.
