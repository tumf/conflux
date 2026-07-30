## ADDED Requirements

### Requirement: Supervised run emits one machine-readable terminal result

`cflx run --supervised` MUST reserve stdout for exactly one compact newline-terminated JSON `run_terminal` record on every controlled exit after supervised-mode initialization. Startup, progress, tracing, warning, error, and cleanup output MUST use stderr. The terminal schema MUST be versioned and MUST NOT be derived by parsing human-readable logs or lifecycle events.

#### Scenario: successful supervised run exits with one result

**Given**: a supervised run reaches typed completion
**When**: bounded cleanup finishes
**Then**: stdout contains exactly one valid `schema_version: 1` `run_terminal` JSON line
**And**: the record outcome is `completed`
**And**: the process exits with status 0
**And**: no human-readable log is mixed into stdout

#### Scenario: controlled failure remains structured

**Given**: supervised mode has initialized its result channel
**And**: startup or orchestration returns a typed fatal failure
**When**: cflx terminates the attempt
**Then**: it emits exactly one terminal record with outcome `failed`
**And**: it exits with status 1

### Requirement: Supervised outcomes and exit codes are deterministic

The terminal outcome MUST be one of `completed`, `blocked`, `stalled`, `cancelled`, or `failed`. Exit codes MUST map respectively to `0`, `2`, `2`, `3`, and `1`. `resumable` MUST come from typed repository/workspace outcome evidence and MUST NOT be inferred from narrative error text.

#### Scenario: resumable terminal hold exits for supervisor retry

**Given**: orchestration reaches a typed blocked or stalled outcome that preserves repository-derived resume evidence
**When**: supervised run handles the outcome
**Then**: the record identifies the typed outcome
**And**: `resumable` is true
**And**: the process exits with status 2 without waiting for an in-process retry request

#### Scenario: graceful signal is cancellation

**Given**: a supervised run receives SIGTERM or SIGINT
**When**: bounded graceful cancellation completes
**Then**: the terminal record outcome is `cancelled`
**And**: the process exits with status 3

#### Scenario: crash is distinguishable from controlled exit

**Given**: the process is killed, aborts, or crashes before terminal emission
**When**: the supervisor observes process termination
**Then**: no valid terminal-record guarantee applies
**And**: absence of a valid record plus process status distinguishes the crash from a cflx-reported outcome

### Requirement: Remote completion requires confirmed upstream publication

When upstream integration is enabled, supervised outcome `completed` MUST require successful final verification, native non-force push, and remote confirmation. A blocked/stalled scheduler result, verification failure, push failure, authentication failure, or unconfirmed remote state MUST NOT produce `completed`.

#### Scenario: remote-confirmed run completes

**Given**: an upstream-enabled supervised run has drained successfully
**And**: final verification, native push, and remote observation succeed
**When**: terminal outcome is built
**Then**: it reports `completed`
**And**: observed remote, branch, local HEAD, and remote HEAD fields identify the confirmed publication

#### Scenario: push failure cannot masquerade as completion

**Given**: final push or remote confirmation fails
**When**: supervised run classifies the typed finalization result
**Then**: it reports `stalled` when repository evidence defines a resumable hold, otherwise `failed`
**And**: it does not emit `completed` or exit 0

### Requirement: Terminal records are privacy-limited observations

The terminal record MUST include stable typed fields for schema, outcome, resumability, selected and classified change IDs, and optional observed repository identity. It MUST NOT include credentials, environment values, prompts, unrestricted command output, terminal buffers, or complete configuration. The record MUST NOT control a later cflx resume decision.

#### Scenario: terminal result contains no secret-bearing payload

**Given**: runtime configuration and command environment contain secrets
**When**: any terminal outcome is serialized
**Then**: only the declared allow-listed fields are present
**And**: failure information is limited to a typed reason code and bounded sanitized detail

#### Scenario: restart ignores previous result for workflow routing

**Given**: a prior supervised attempt emitted a terminal record
**When**: cflx starts again from the persistent checkout
**Then**: it derives the next action from workspace files, Git state, and base-tree comparison
**And**: deleting or changing the prior terminal observation does not alter routing

### Requirement: Ordinary run and lifecycle behavior remain compatible

Without `--supervised`, `cflx run` MUST retain its existing logging and operator/web retry behavior. The external lifecycle adapter MUST remain observability-only and MAY remain lossy; adapter spawn, write, backpressure, or shutdown failure MUST NOT change supervised terminal classification or exit code.

#### Scenario: default run retains retry loop

**Given**: `cflx run` is invoked without `--supervised`
**When**: orchestration returns an error under the existing retry-capable run path
**Then**: existing operator/web retry behavior remains available
**And**: no `run_terminal` stdout contract is enabled

#### Scenario: broken lifecycle adapter does not change result

**Given**: supervised run uses a missing, crashed, or non-reading lifecycle adapter
**When**: orchestration reaches a terminal outcome
**Then**: the same terminal record and exit status are produced as without the adapter
