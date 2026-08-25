## ADDED Requirements

### Requirement: Wait releases settled and manual-action statuses

`cflx client wait <change-id>` MUST continue observing only while the current coherent owner state can still advance the requested change without a new operator command. Active lifecycle phases and recoverable external-condition holds MAY continue waiting. A final status or a status that requires a new operator action MUST release the wait immediately with a stable typed result. This classification MUST run on the initial observation and every later coherent observation.

An observed `merged` status MUST still use repository certification before returning successful `completed`. If its repository evidence does not certify success, wait MUST return a typed non-success result rather than hold for another owner event. An observed `error`, `merge wait`, `stopped`, or equivalent final/manual-action status MUST return a stable non-success outcome carrying the observed status and available error detail. Existing rejection, fatal-process, evidence, owner-replacement, and timeout outcomes MUST retain their narrower meanings. No release path may submit a workflow command.

#### Scenario: Existing error releases immediately

**Given**: the initial coherent snapshot shows `alpha` at `error`
**When**: an agent runs `cflx client wait alpha --json` without a timeout
**Then**: the command exits immediately with a typed non-success outcome containing status `error`
**And**: it submits no retry or other command

#### Scenario: Existing merge wait releases immediately

**Given**: the initial coherent snapshot shows `alpha` at `merge wait`
**When**: an agent runs `cflx client wait alpha --json` without a timeout
**Then**: the command exits immediately with a typed non-success outcome containing status `merge wait`
**And**: it submits no resolve, merge, or other command

#### Scenario: Transition into manual action releases a running wait

**Given**: wait is observing `alpha` in an automatically progressing phase
**When**: a later coherent snapshot shows `error`, `merge wait`, or `stopped`
**Then**: wait exits on that observation with the corresponding typed non-success result
**And**: it does not wait for another activity event

#### Scenario: Existing merged status is classified immediately

**Given**: the initial coherent snapshot shows `alpha` at `merged`
**When**: wait evaluates repository completion evidence
**Then**: it returns `completed` if the evidence certifies the owner's terminal contract
**And**: otherwise it returns a typed non-success evidence or settled-status result without holding for a future owner event

#### Scenario: Automatically progressing status remains held

**Given**: `alpha` is applying, accepting, archiving, merging, resolving, queued, or waiting on a recoverable external condition that can clear without a new operator command
**When**: wait observes that status and no other terminal condition exists
**Then**: wait continues observing
**And**: it does not synthesize a failure or submit a command

#### Scenario: Release remains observation only

**Given**: any final or manual-action status releases the wait
**When**: the result envelope is produced
**Then**: `detail.commands_submitted` is zero
**And**: no start, retry, queue, resolve, archive, merge, cleanup, or worktree command was submitted
