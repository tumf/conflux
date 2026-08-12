## ADDED Requirements

### Requirement: Local client compatibility discovery

The versioned single-instance API MUST expose enough generated capability, instance, authoritative-state, execution-status, command-record, and event information for the source-matched `cflx client` client to inspect and operate a command-capable owner without parsing logs or display strings. The client MUST check API compatibility and process incarnation before mutation and during command settlement. A typed command-capability field MUST identify whether the executor is bound. An unbound command executor MUST return a distinct `command_executor_unbound` wire error rather than an ordinary lifecycle conflict or a command queued for later execution.

This compatibility surface MUST include a minimal typed owner execution contract at an `instance_id` and `state_revision`: base branch identity and terminal mode (`merged`, `base_published`, or `branch_pushed`), plus selected remote and pushed branch when applicable. The generated OpenAPI document MUST publish these fields. They are observational facts only and MUST NOT become durable workflow authority; repository and workspace evidence remain authoritative for routing and truthful completion.

#### Scenario: Source-matched client discovers required behavior

**Given**: a command-capable owner serves `/api/v2`
**When**: the same build's client reads capabilities, instance, state, and execution status
**Then**: it can determine supported observation and command behavior from typed fields
**And**: it does not need to parse logs, prose details, or TUI presentation strings

#### Scenario: Owner contract identifies truthful terminal proof

**Given**: an owner integrates changes locally or publishes them to a selected remote
**When**: the client reads the owner execution contract at the matching state revision
**Then**: it receives the base branch identity and terminal success mode
**And**: `base_published` identifies the selected remote
**And**: `branch_pushed` identifies the selected remote and pushed branch without claiming base integration
**And**: the generated OpenAPI contract describes these typed fields

#### Scenario: Multi-resource observation rejects mixed revisions

**Given**: owner state advances while a client reads state, execution status, and owner execution contract
**When**: the returned incarnation or revision boundaries do not agree
**Then**: the client cannot treat the values as one coherent observation
**And**: bounded reread or a typed observation conflict is required before mutation or completion reporting

#### Scenario: Incompatible API fails before mutation

**Given**: the discovered owner lacks a required route, command, field, or compatible API version
**When**: the client prepares an enqueue operation
**Then**: it returns an incompatible-owner outcome
**And**: no mutation command is submitted

#### Scenario: Unbound executor remains fail closed

**Given**: a process serves read resources but has no bound command executor
**When**: the client submits or prepares a mutation
**Then**: typed command capability reports that mutation is unavailable
**And**: command execution is refused with `command_executor_unbound`
**And**: the request is not queued for a future executor

### Requirement: Client observation does not alter API semantics

Serving the local client MUST reuse the existing `/api/v2` router, DTOs, optimistic revision rules, idempotent command records, and shared operator application transaction. The API MUST NOT add a second orchestration path, client-specific hidden mutation, or durable run record solely for `cflx client`.

#### Scenario: Enqueue uses ordinary typed commands

**Given**: the client admits an eligible change
**When**: it submits mutations to the owner
**Then**: every mutation appears as an ordinary v2 command record
**And**: TUI and API projections observe the same shared operator outcomes

#### Scenario: Status and wait are read only

**Given**: a client invokes client status or wait
**When**: it reads snapshots, execution status, events, and repository evidence
**Then**: no v2 command record is created by those operations
**And**: no process-local mark, queue, scheduler, resolver, cancellation, or mode state changes because of observation
