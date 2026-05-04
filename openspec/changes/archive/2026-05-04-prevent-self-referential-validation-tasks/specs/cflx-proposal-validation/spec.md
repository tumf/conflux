## ADDED Requirements

### Requirement: self-referential-final-validation-task-guard

The native `cflx openspec validate` implementation SHALL reject checkbox tasks that require final OpenSpec validation of the same change as implementation evidence. Final OpenSpec validation is an archive gate, not an implementation checkbox task.

The validator SHALL allow non-checkbox final validation guidance sections to mention the same validation command.

#### Scenario: same-change final validation checkbox fails

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains a checkbox task that asks the implementer to run or record `cflx openspec validate alpha`
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: validation fails
**And**: the diagnostic identifies the task as a self-referential final validation checkbox
**And**: the diagnostic tells the author to move final validation to a non-checkbox `Final Validation` section

#### Scenario: non-checkbox final validation section passes

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains a non-checkbox `## Final Validation` section mentioning `cflx openspec validate alpha --strict --evidence warn`
**And**: all implementation checkbox tasks have valid repository-verifiable evidence notes
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: validation passes without a self-referential final validation diagnostic

#### Scenario: ordinary repository evidence remains accepted

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains implementation checkbox tasks verified by runnable commands such as `cargo test`, `npm run`, `go test`, or source/test file paths
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: those ordinary verification notes remain accepted

### Requirement: archive-equivalent-validation-command

Conflux SHALL provide a documented local validation command whose failure policy matches archive readiness. Users and agents MUST be able to reproduce archive-blocking evidence findings before invoking archive.

#### Scenario: local archive gate fails on evidence findings

**Given**: an active change `alpha` has an evidence finding that would block archive
**When**: the documented archive-equivalent validation command is executed locally
**Then**: the command exits non-zero
**And**: it reports the same evidence finding that would block archive

#### Scenario: archive failure message includes local reproduction command

**Given**: archive fails because validation found evidence issues
**When**: Conflux reports the archive failure
**Then**: the message includes the local archive-equivalent validation command for the same change
**And**: the message preserves the specific validation finding instead of reporting only generic archive verification failure
