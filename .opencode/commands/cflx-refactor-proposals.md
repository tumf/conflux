---
agent: build
description: "Scan for refactor candidates and draft Conflux (OpenSpec) proposals for each"
---

The user wants you to find refactoring opportunities in the current repository, then create one Conflux (OpenSpec) change proposal per opportunity under `openspec/changes/<change-id>/`.

<UserRequest>
$ARGUMENTS
</UserRequest>

CRITICAL RESTRICTIONS
- This command is PROPOSAL CREATION ONLY.
- DO NOT implement or modify product/source code.
- DO NOT edit files outside `openspec/changes/`.
- You MAY read any files for analysis.
- Prefer small, focused proposals. Default to 3 proposals unless the user explicitly asks for more.

Language rules
- `openspec/changes/**` files MUST be written in Japanese.
- All other files must remain untouched.

Repository compatibility check
1. If `openspec/` does not exist, STOP and tell the user this repo is not Conflux/OpenSpec-initialized.
2. If `python3 "<SKILL_ROOT>/scripts/cflx.py" list --specs` fails because `"<SKILL_ROOT>/scripts/cflx.py"` does not exist, still draft the proposals (files under `openspec/changes/`), but skip validation and mention the missing validator.

Discovery (find refactor candidates)
Collect evidence first. Use fast, low-risk heuristics; prioritize areas with clear payoff and low functional risk.

Run targeted searches (adjust to the repo language mix):
- TODO/FIXME/DEBT: `rg -n "TODO|FIXME|HACK|XXX|DEBT|REFACTOR" -S .`
- Lint/type bypasses: `rg -n "(eslint-disable|tslint:disable|pylint: disable|noqa|type: ignore|@ts-ignore)" -S .`
- Suspicious error handling: `rg -n "catch\s*\(.*\)\s*\{\s*\}" -S .` (where applicable)
- Risky patterns:
  - Rust: `rg -n "\b(unwrap|expect)\(" -S .`
  - JS/TS: `rg -n "\bas any\b" -S .`
  - Python: `rg -n "\bassert False\b" -S .`

Also identify a few large/central files by line count among tracked files (do not scan vendored artifacts).

Candidate selection and grouping
- Create a ranked list of candidates with: file path(s), what the smell is, and 1-3 concrete evidence snippets (line references).
- De-duplicate related hits into a single opportunity (e.g., same module, same pattern, shared root cause).
- Pick the top 3 (default) and explain why these are the best refactor targets.

For each selected opportunity, create one change proposal
- Choose a unique verb-led change id prefixed with `refactor-` (kebab-case). Examples: `refactor-http-client`, `refactor-config-loading`.
- Create these files under `openspec/changes/<id>/`:
  - `proposal.md`
  - `tasks.md`
  - `design.md` (ONLY if cross-cutting or needs trade-off documentation)
  - `specs/<capability>/spec.md`

Proposal requirements
- Refactoring should aim for no functional behaviour change by default.
- Include acceptance criteria that focuses on stability/regression prevention, e.g. test suite passes, no API/CLI changes, performance non-regression where relevant.
- In `tasks.md`, include characterization tests BEFORE refactor (verification steps required for each task).
- In `spec.md`, include at least one minimal, testable requirement/scenario so strict validation can pass even for "no intended behaviour change" refactors.

Validation
- If `"<SKILL_ROOT>/scripts/cflx.py"` is present, run `python3 "<SKILL_ROOT>/scripts/cflx.py" validate <id> --strict` for EACH proposal and fix any issues.

Output to the user
- Show the list of proposals created with their change IDs and paths.
- For each proposal, include a 1-2 sentence Japanese abstract (what/why) and key acceptance criteria.
