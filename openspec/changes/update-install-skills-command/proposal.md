# Change: Simplify install-skills to implicit bundled source

## Problem/Context

- The current `install-skills` interface requires a `<SOURCE>` argument even though the intended and practical workflow only uses bundled Conflux skills.
- The current CLI spec, implementation, tests, and README all document `self` and `local:<path>` even though those source choices add complexity without a meaningful user-facing benefit.
- The user wants `cflx install-skills` to default to project installation and `cflx install-skills --global` to switch to global installation, with no required source parameter.

## Proposed Solution

- Change the command shape from `cflx install-skills <SOURCE> [--global]` to `cflx install-skills [--global]`.
- Treat bundled skills from the repository `skills/` directory as the only install source.
- Keep scope selection simple: default to project install, and use `--global` for global install.
- Reject legacy source-bearing forms such as `cflx install-skills self` and `cflx install-skills local:<path>` with an explicit error that points users to the new syntax.

## Acceptance Criteria

- Running `cflx install-skills` installs bundled skills into `./.agents/skills` and writes `./.agents/.skill-lock.json`.
- Running `cflx install-skills --global` installs bundled skills into `~/.agents/skills` and writes `~/.agents/.skill-lock.json`.
- The CLI no longer accepts a positional source argument for `install-skills`.
- Legacy source-bearing invocations fail with guidance to use `cflx install-skills` or `cflx install-skills --global`.
- User-facing documentation and tests reflect the simplified command.

## Out of Scope

- Adding new remote or local skill source types.
- Changing the bundled skill packaging layout under the repository `skills/` directory.
