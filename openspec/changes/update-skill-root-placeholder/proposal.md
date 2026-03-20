# Change: Clarify skill root placeholder notation

## Problem/Context

- The bundled skill documentation under `skills/` currently uses `$SKILL_ROOT` in command examples.
- That notation looks like a shell environment variable even though the skill runtime treats it as a documentation placeholder for the skill's base directory.
- The user wants all `skills/` content to use `<SKILL_ROOT>` instead so the placeholder reads as descriptive text instead of shell expansion syntax.

## Proposed Solution

- Replace `$SKILL_ROOT` with `<SKILL_ROOT>` across the skill markdown files under `skills/`.
- Update surrounding wording where needed so examples clearly describe `<SKILL_ROOT>` as a placeholder path segment rather than an exported environment variable.
- Keep the change documentation-only; do not alter the bundled Python helper layout or the behavior of `install-skills`.

## Acceptance Criteria

- All markdown files under `skills/` use `<SKILL_ROOT>` instead of `$SKILL_ROOT` when referring to the skill base directory placeholder.
- Command examples in `skills/README.md`, `skills/cflx-proposal/SKILL.md`, and `skills/cflx-workflow/SKILL.md` consistently show `python3 "<SKILL_ROOT>/scripts/cflx.py" ...`.
- The updated wording makes it clear that `<SKILL_ROOT>` is placeholder notation and not a shell environment variable users are expected to define.

## Out of Scope

- Changing runtime behavior for skill installation or execution.
- Renaming other placeholder syntaxes outside `skills/`.
