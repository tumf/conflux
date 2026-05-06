# Design: SPECA runner adapter guidance for acceptance

## Design Goals

- Make official NyxFoundation/speca execution an attempted supporting proof path when it is locally usable.
- Keep Conflux acceptance deterministic from workspace/git evidence and the standard verdict contract.
- Avoid target worktree pollution from cloned tools, generated inputs, outputs, logs, or failed runner attempts.
- Ensure runner unavailability degrades to manual SPECA-style review instead of acceptance success.

## Adapter Boundary

This change updates the bundled acceptance skill guidance only. It does not add a new Conflux subcommand, runner binary, parser branch, or durable SPECA state model.

The skill should guide the agent to treat official SPECA as an external helper:

1. Inspect prerequisites.
2. Prepare temporary inputs outside the target repo.
3. Execute the runner from the SPECA checkout when possible.
4. Read produced outputs as evidence.
5. Map evidence to normal acceptance findings.

## Filesystem Layout

Recommended default locations:

- SPECA checkout/cache: `~/tmp/speca`
- Generated Conflux/OpenSpec input bundle: `~/tmp/speca-conflux-input/<change-id>/`
- Official SPECA outputs/logs: inside the SPECA checkout or `~/tmp/speca-conflux-output/<change-id>/`

These paths are outside the Conflux repository and therefore cannot become authoritative workflow-control inputs. The reviewer may cite runner outputs in reasoning, but pass/fail/gated/continue decisions must still be grounded in repository files, git state, specs, tasks, tests, and changed code.

## Runner Command Guidance

Because SPECA command-line details may evolve, the skill should not freeze every flag value as a Conflux protocol. It should require the agent to inspect the installed SPECA checkout and then run the documented phase command shape from that checkout, centered on:

```bash
agent-exec run -- uv run python3 scripts/run_phase.py ...
```

The ellipsis must be replaced with the phase and arguments supported by the checked-out SPECA version. If command help or repository docs contradict older examples, the installed checkout's docs/help win.

## Failure Classification

- Runner completes and produces relevant outputs: use outputs as supporting proof/falsification evidence, then map any concrete property failure to standard acceptance `fail` findings.
- Runner prerequisites missing or auth unavailable: record the limitation in human-readable reasoning, then perform manual SPECA-style review.
- Runner crashes or produces unusable output: record the failed command and output location, then perform manual SPECA-style review.
- Runner output conflicts with repository evidence: repository evidence and OpenSpec requirements are authoritative for acceptance.

## Verification Strategy

Add embedded-skill contract tests so the implementation is verifiable without running external SPECA:

- test that the skill documents NyxFoundation/speca and `uv run python3 scripts/run_phase.py`;
- test that the skill keeps runner artifacts outside the target worktree;
- test that it requires fallback when runner execution is unavailable;
- reuse existing drift tests to ensure no fixed acceptance checklist or SPECA-specific terminal marker is introduced.
