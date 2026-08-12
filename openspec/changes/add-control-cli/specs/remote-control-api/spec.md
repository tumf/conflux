## ADDED Requirements

### Requirement: Local control client compatibility discovery

The versioned single-instance API MUST expose enough generated capability, instance, authoritative-state, execution-status, command-record, and event information for the source-matched `cflx control` client to inspect and operate a command-capable owner without parsing logs or display strings. The client MUST check API compatibility and process incarnation before mutation and during command settlement. An unbound command executor MUST remain a typed fail-closed condition rather than queueing commands for later execution.

This compatibility surface MUST NOT make API state durable workflow authority. Repository and workspace evidence remain authoritative for routing and truthful completion.

#### Scenario: Source-matched client discovers required behavior

**Given**: a command-capable owner serves `/api/v2`
**When**: the same build's control client reads capabilities, instance, state, and execution status
**Then**: it can determine supported observation and command behavior from typed fields
**And**: it does not need to parse logs, prose details, or TUI presentation strings

#### Scenario: Incompatible API fails before mutation

**Given**: the discovered owner lacks a required route, command, field, or compatible API version
**When**: control prepares an enqueue operation
**Then**: it returns an incompatible-owner outcome
**And**: no mutation command is submitted

#### Scenario: Unbound executor remains fail closed

**Given**: a process serves read resources but has no bound command executor
**When**: the control client submits or prepares a mutation
**Then**: command execution is refused with typed lifecycle information
**And**: the request is not queued for a future executor

### Requirement: Control-client observation does not alter API semantics

Serving the local control client MUST reuse the existing `/api/v2` router, DTOs, optimistic revision rules, idempotent command records, and shared operator application transaction. The API MUST NOT add a second orchestration path, client-specific hidden mutation, or durable run record solely for `cflx control`.

#### Scenario: Enqueue uses ordinary typed commands

**Given**: control admits an eligible change
**When**: it submits mutations to the owner
**Then**: every mutation appears as an ordinary v2 command record
**And**: TUI and API projections observe the same shared operator outcomes

#### Scenario: Status and wait are read only

**Given**: a client invokes control status or wait
**When**: it reads snapshots, execution status, events, and repository evidence
**Then**: no v2 command record is created by those operations
**And**: no process-local mark, queue, scheduler, resolver, cancellation, or mode state changes because of observation
