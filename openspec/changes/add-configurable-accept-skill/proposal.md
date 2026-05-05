---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/configuration/spec.md
  - openspec/specs/agent-prompts/spec.md
  - src/config/types.rs
  - src/agent/prompt.rs
  - src/embedded_skills.rs
  - skills/cflx-accept/SKILL.md
  - .opencode/commands/cflx-accept.md
---

# Change: Add configurable acceptance skill

**Change Type**: implementation

## Premise / Context

- Acceptance prompt construction currently hardcodes `load skills: cflx-accept` in `src/agent/prompt.rs`.
- Conflux already keeps fixed acceptance procedure ownership in `.opencode/commands/cflx-accept.md`, while `skills/cflx-accept/SKILL.md` provides operation identity and scoped guidance.
- The requested behavior is to add a config field `accept_skill` with default `cflx-accept` so projects can choose an alternate acceptance skill without replacing the whole `acceptance_command`.
- A built-in `cflx-accept-with-speca` skill should be provided so users can opt in with `"accept_skill": "cflx-accept-with-speca"`.
- This change must preserve the existing JSON acceptance verdict contract and workspace-local workflow-state constitution.

## Requested Artifact

Implementation proposal for configurable acceptance skill selection plus a built-in SPECA-oriented acceptance skill.

## Problem

Today, acceptance customization has a coarse boundary: users can replace `acceptance_command` or modify command templates, but the variable acceptance prompt always asks the agent to load `cflx-accept`. This makes it harder to experiment with alternate acceptance review policies such as SPECA-backed proof attempts while preserving the standard acceptance command, diff context, retry history, and verdict parser.

## Proposed Solution

Add an optional top-level config key:

```jsonc
{
  "accept_skill": "cflx-accept"
}
```

Behavior:

1. If `accept_skill` is omitted, Conflux behaves exactly as today and emits `load skills: cflx-accept`.
2. If `accept_skill` is set, acceptance prompt construction emits `load skills: <configured-name>`.
3. Config merge precedence for `accept_skill` follows the existing `.cflx.jsonc` / global / custom config precedence rules.
4. The configured value is used only as acceptance prompt guidance; it must not alter workflow-control state, acceptance verdict parsing, archive routing, or command execution semantics by itself.
5. Conflux ships a built-in skill `cflx-accept-with-speca` that extends acceptance guidance with SPECA-oriented property/proof-attempt review while preserving the same final verdict contract.

`cflx-accept-with-speca` should be additive:

- load or reference the normal `cflx-accept` expectations instead of redefining incompatible verdict rules
- use OpenSpec deltas and acceptance diff context to select/generate relevant SPECA-style properties
- run SPECA proof-attempts when the environment/tooling is available, or perform an equivalent structured property review when not available
- map blocking property failures to the existing JSON verdict format
- keep the final verdict as exactly one Conflux acceptance verdict

## Acceptance Criteria

1. A config with no `accept_skill` produces an acceptance prompt containing `load skills: cflx-accept`.
2. A config with `"accept_skill": "cflx-accept-with-speca"` produces an acceptance prompt containing `load skills: cflx-accept-with-speca` and not the default skill prelude.
3. `accept_skill` is loaded, merged, and overridden with the same precedence rules as other top-level optional config fields.
4. The generated acceptance prompt still includes change metadata, paths, diff context, archive readiness context, previous acceptance output, user acceptance prompt, and history in the existing order after the selected skill prelude.
5. The bundled skill inventory includes `cflx-accept-with-speca`, and embedded-skill tests prove it is available when Conflux installs or exposes built-in skills.
6. `cflx-accept-with-speca` preserves the existing JSON verdict contract: `pass`, `fail`, `continue`, or `gated` with actionable `findings` on fail.
7. The change does not require users to replace `acceptance_command` to opt into SPECA-style acceptance behavior.

## Explicit Completion Conditions

- `OrchestratorConfig` includes optional `accept_skill` storage, merge behavior, accessor/default behavior, and tests.
- Acceptance prompt construction accepts the selected skill name rather than hardcoding `cflx-accept`.
- Call sites that build acceptance prompts pass the configured skill name through without changing acceptance command execution or verdict parsing.
- Built-in skill files include `skills/cflx-accept-with-speca/SKILL.md` and embedded skill registration/tests cover it.
- Template or config documentation mentions `accept_skill` with default `cflx-accept` and opt-in example `cflx-accept-with-speca`.
- Targeted tests for config merge/defaults, prompt construction, and embedded skill inventory pass.
- `cflx openspec validate add-configurable-accept-skill --strict --evidence warn` passes.

## Out of Scope

- Implementing a first-class `cflx speca` subcommand.
- Adding `speca-accept` as an external binary or changing `acceptance_command` semantics.
- Adding `{prompt_file}` / stdin handoff for acceptance prompts.
- Changing the acceptance JSON verdict parser or legacy verdict compatibility.
- Persisting SPECA proof traces in acceptance history or adding dashboard-first-class SPECA finding UI.
