## ADDED Requirements

### Requirement: Skills Documentation Placeholder Clarity

Documentation under `skills/` SHALL represent the bundled skill root path placeholder as `<SKILL_ROOT>` in prose and command examples so readers do not mistake it for a shell environment variable.

#### Scenario: Skill command examples use placeholder notation

- **WHEN** a reader reviews command examples in `skills/README.md` or bundled `skills/*/SKILL.md` files
- **THEN** any example path to the bundled `scripts/cflx.py` helper uses `<SKILL_ROOT>`
- **AND** the examples do not use `$SKILL_ROOT`

#### Scenario: Placeholder meaning is explicit

- **WHEN** a reader encounters `<SKILL_ROOT>` in the `skills/` documentation
- **THEN** the surrounding wording makes clear that it denotes the installed skill's root directory
- **AND** the documentation does not instruct the reader to define a shell variable named `SKILL_ROOT`
