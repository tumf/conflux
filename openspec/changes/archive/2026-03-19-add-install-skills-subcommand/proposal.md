# Change: Add install-skills subcommand

## Problem / Context

Conflux currently has no CLI path for installing bundled or local agent skills into the standard `.agents/skills` locations. The user wants `cflx` to match the `agent-exec` approach, where `self` installs bundled skills from a top-level `skills/` layout and `local:<path>` installs from a local skills source.

Relevant repository context:
- The existing CLI is defined in `src/cli.rs` and dispatched in `src/main.rs`.
- Current CLI capabilities are documented in `openspec/specs/cli/spec.md`.
- The repository does not currently contain an `install-skills` subcommand or an `agent-skills-rs` dependency.
- The repo currently stores one local skill under `.agents/skills/refactor`, but the requested bundled-source behavior should follow the `agent-exec` convention of a top-level `skills/` directory for `self` installs.

## Proposed Solution

Add a new `cflx install-skills <source> [--global]` subcommand backed by `agent-skills-rs`.

The command will:
- Accept `self` and `local:<path>` as the only supported source forms.
- Treat `self` as bundled skills discovered from the repository's top-level `skills/` layout.
- Install into project scope by default: `./.agents/skills` with lock file `./.agents/.skill-lock.json`.
- Install into global scope with `--global`: `~/.agents/skills` with lock file `~/.agents/.skill-lock.json`.
- Fail fast for unsupported source schemes and list the allowed forms in the error message.

## Acceptance Criteria

- `cflx install-skills self` is accepted by the CLI and routes to an installation flow.
- `cflx install-skills local:<path>` is accepted by the CLI and uses the same install pipeline.
- Unknown source schemes are rejected with an explicit allowed-schemes message.
- Project-scope installs write skills under `.agents/skills` and update `.agents/.skill-lock.json`.
- Global installs write skills under `~/.agents/skills` and update `~/.agents/.skill-lock.json`.
- Bundled skills for `self` are sourced from a top-level `skills/` layout, aligned with `agent-exec`.
- Automated tests cover CLI parsing and project/global installation behavior.

## Out of Scope

- Adding CLI introspection features such as `commands --output json` or `schema --command ...`.
- Supporting remote skill sources beyond `self` and `local:<path>`.
- Implementing a separate uninstall or update command in this proposal.
