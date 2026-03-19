# Change: Remove outdated approval-flow references from documentation

## Problem/Context
The approval workflow was removed in the archived change `remove-approval-flow`, but user-facing documentation still describes `@`-based approval and approve/unapprove concepts. This creates a false operating model for new users, especially in README-driven onboarding.

A quick repo scan shows outdated approval references at least in:
- `README.md`
- `docs/guides/USAGE.md`
- `docs/guides/DEVELOPMENT.md`

Related localization and API docs should also be reviewed so the documentation set stays internally consistent.

## Proposed Solution
Update the documentation set to consistently describe the post-approval workflow:
- remove `@`-key approval instructions from README and usage guides
- describe selection/queue behavior without an approval state
- remove or rewrite references to approve/unapprove APIs, hooks, and examples when they are no longer part of the current product
- review `README.ja.md` and other linked docs for the same outdated model and synchronize them with the corrected English docs

## Acceptance Criteria
- README no longer instructs users to use `@` for approval or describes approval as a current change state
- Usage and development guides no longer mention removed approval interactions as active behavior
- README.ja.md and other reviewed docs stay consistent with README.md regarding the current workflow
- Documentation examples and terminology match the current CLI/TUI/Web behavior after approval-flow removal

## Out of Scope
- Reintroducing or redesigning an approval workflow
- Changing runtime behavior, CLI commands, TUI keybindings, or web APIs
- Refactoring unrelated documentation text that is already accurate
