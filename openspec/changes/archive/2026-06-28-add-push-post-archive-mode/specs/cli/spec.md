## MODIFIED Requirements

### Requirement: run Subcommand

The `run` subcommand SHALL execute the OpenSpec change workflow orchestration loop.

When `--push [remote]` is provided with parallel execution, `run` SHALL use push post-archive mode instead of base-merge post-archive mode. If the remote argument is omitted, the remote SHALL default to `origin`. The remote argument MUST NOT contain `:`; branch selection is unsupported and MUST be rejected before orchestration starts.

<!-- Expected canonical result after archive: `cli` will document `cflx run --push [remote]` as an opt-in parallel post-archive push mode with default remote `origin` and no branch override syntax. -->

#### Scenario: Run push mode defaults to origin

- **WHEN** user runs `cflx run --parallel --push`
- **THEN** run mode is configured for push post-archive action
- **AND** the selected remote is `origin`
- **AND** completed change branches are not configured for base-branch merge

#### Scenario: Run push mode accepts remote name

- **WHEN** user runs `cflx run --parallel --push upstream`
- **THEN** run mode is configured for push post-archive action
- **AND** the selected remote is `upstream`

#### Scenario: Run push mode rejects branch selection

- **WHEN** user runs `cflx run --parallel --push origin:main`
- **THEN** orchestration does not start
- **AND** the CLI reports that branch selection is not supported for `--push`
- **AND** no worktree, apply, acceptance, archive, merge, or push work is started
