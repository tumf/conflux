## Implementation Tasks

- [ ] 1. Audit user-facing docs for approval-flow remnants, including `README.md`, `README.ja.md`, `docs/guides/USAGE.md`, and `docs/guides/DEVELOPMENT.md` (verification: repository search shows no stale user-facing instructions such as `Use @ to approve`, `approve/unapprove` workflow descriptions, or approval-state tables in these files)
- [ ] 2. Update `README.md` to describe current change selection/queue behavior without approval terminology (verification: `README.md` no longer documents `@` as an active keybinding or approval as a prerequisite state)
- [ ] 3. Update `README.ja.md` to match the corrected README workflow and examples (verification: command examples and workflow descriptions remain aligned between `README.md` and `README.ja.md`)
- [ ] 4. Update `docs/guides/USAGE.md` and `docs/guides/DEVELOPMENT.md` to remove removed approval interactions and rewrite any affected guidance in terms of current behavior (verification: both files describe only current commands, keybindings, hooks, and APIs)
- [ ] 5. Re-scan documentation and validate the proposal for consistency (verification: `rg -n "Use @ to approve|approve a change|unapprove|approval state|Toggle approval|on_approve|on_unapprove" README.md README.ja.md docs/guides` returns only intentionally retained historical or internal references, if any; `openspec validate update-docs-remove-approval-references --strict --no-interactive` passes)

## Future Work

- Review deeper audit/reference docs outside the main user onboarding surface if we later want the entire repository to be free of historical approval terminology.
