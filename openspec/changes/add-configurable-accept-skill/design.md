# Design: Configurable acceptance skill

## Current Behavior

Acceptance prompt construction currently includes a fixed prelude:

```text
load skills: cflx-accept
```

The rest of the prompt contains variable context: change id, paths, diff context, archive readiness context, previous output, user acceptance prompt, and history. Fixed acceptance checklist and verdict formatting are owned by `.opencode/commands/cflx-accept.md`.

## Proposed Design

Introduce `accept_skill` as an optional top-level config value with default `cflx-accept`.

Implementation shape:

1. Add `accept_skill: Option<String>` to `OrchestratorConfig`.
2. Include `accept_skill` in config merge behavior.
3. Add an accessor such as `get_accept_skill() -> &str` that returns `cflx-accept` when unset or empty policy rejects empty values explicitly.
4. Update acceptance prompt construction to receive the selected skill name and render:

```text
load skills: <accept_skill>
```

5. Update call sites so the acceptance runner obtains the selected skill from config and passes it into the prompt builder.
6. Keep acceptance command execution, `{prompt}` expansion, streaming output, early verdict detection, and verdict parsing unchanged.

## Built-in SPECA skill

Add `skills/cflx-accept-with-speca/SKILL.md` as a built-in acceptance-mode skill.

The SPECA skill should not replace Conflux's verdict protocol. It should add an extra review lens:

- derive or select properties from OpenSpec deltas and changed files
- attempt to falsify those properties against relevant implementation paths
- classify findings as blocking, advisory, incomplete, or gated
- emit only the existing Conflux JSON verdict at the end

This lets users opt in by configuration:

```jsonc
{
  "accept_skill": "cflx-accept-with-speca"
}
```

without replacing `acceptance_command`.

## Ownership Boundaries

- `accept_skill` selects the skill prelude only.
- `.opencode/commands/cflx-accept.md` remains the fixed procedure and verdict-format single source unless a future change intentionally generalizes command templates too.
- `cflx-accept-with-speca` must not require durable workflow state outside the workspace.
- SPECA execution failures should map to `continue`, `fail`, or advisory findings according to the skill guidance, but they must still use the existing final verdict contract.

## Risks and Mitigations

- **Invalid skill name**: Treat the value as prompt text rather than resolving it at config-load time, or validate only basic non-empty/safe-token constraints. Full skill availability may depend on the agent runtime.
- **Prompt injection via config**: Keep `accept_skill` constrained to a skill identifier pattern such as `[A-Za-z0-9_.:/-]+` if needed, because it is interpolated into prompt text.
- **Procedure drift**: Keep fixed checklist and verdict formatting out of `cflx-accept-with-speca`; the SPECA skill adds review strategy, not a conflicting protocol.
- **Heavy SPECA runs**: The skill should allow advisory/property review fallback when SPECA tooling is unavailable, and future work can move execution into `speca-accept`.

## Verification Strategy

- Unit-test config default/override/merge.
- Unit-test prompt builder with default and custom skill names.
- Regression-test acceptance parser and streaming behavior to prove verdict handling is unchanged.
- Embedded-skill tests confirm the new built-in skill is registered.
- Manual review confirms the new skill preserves the single final verdict protocol.
