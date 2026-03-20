## Implementation Tasks

- [x] Replace `$SKILL_ROOT` with `<SKILL_ROOT>` in markdown files under `skills/` that reference the bundled `cflx.py` helper (verification: `rg -n '\$SKILL_ROOT|<SKILL_ROOT>' skills` shows no remaining `$SKILL_ROOT` matches and expected `<SKILL_ROOT>` references in `skills/README.md`, `skills/cflx-proposal/SKILL.md`, and `skills/cflx-workflow/SKILL.md`).
- [x] Adjust nearby prose in the affected skill docs so the placeholder is described as the skill root path notation rather than a user-defined environment variable (verification: read the updated sections in `skills/README.md`, `skills/cflx-proposal/SKILL.md`, and `skills/cflx-workflow/SKILL.md`).
- [x] Run strict OpenSpec validation for the proposal after authoring the documentation change plan (verification: `openspec validate update-skill-root-placeholder --strict --no-interactive`).

## Future Work

- Audit non-skill documentation in a separate change if other placeholder notations also need normalization.
