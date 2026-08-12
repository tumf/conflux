---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - src/cli.rs
  - src/main.rs
  - src/repo_lock.rs
  - src/web/remote_control_api
  - tests/openapi_cli_tests.rs
verifications:
  - id: control-cli-tests
    requirement: External agents can inspect, enqueue, and wait for work through a stable CLI without owning orchestration or constructing internal v2 command envelopes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: CLI parser, Unix-socket integration, concurrency-retry, owner-unavailable, read-only status, enqueue, and wait outcome tests
    rerun: cargo test --features web-monitoring --test control_cli_tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add a stable CLI for controlling an existing Conflux owner

**Change Type**: implementation

## Premise / Context

- `cflx run` owns and executes a finite explicit-target orchestration run; it is not a client for an owner already holding the repository lock.
- Local `cflx`, `cflx tui`, and `cflx run` expose `/api/v2` on `${GIT_COMMON_DIR}/cflx-api.sock`, but a headless `cflx run` intentionally has no command executor and rejects mutations.
- The v2 mutation contract requires callers to understand `state_revision`, typed command envelopes, idempotency keys, command settlement, and mode-aware mark/queue/start behavior.
- Agents currently have to reproduce those internal protocol details, making integrations brittle when the API or orchestration state model changes.
- No related active proposal, open PR, or unmerged implementation branch was found; archived work provides the existing socket and v2 control foundation.

## Requested Artifact

Implementation of a stable, machine-readable `cflx control` CLI namespace.

## Problem / Context

An external agent should be able to delegate implementation to an existing interactive Conflux owner without becoming another orchestration owner and without implementing the v2 protocol itself. Direct calls to `/api/v2` leak optimistic-concurrency, idempotency, lifecycle, and queue semantics into every caller. Reusing `cflx run` for this purpose would overload its existing finite-owner contract and can conflict with the repository lock.

The client also needs truthful waiting semantics. A successful enqueue command only proves admission; it does not prove implementation, acceptance, archive, or integration. Conversely, owner disappearance, owner restart, terminal rejection, process failure, and timeout must not be reported as completion.

## Proposed Solution

Add a `cflx control` namespace for a thin local client of the existing repository owner:

- `cflx control status --json` discovers the repository-scoped Unix socket, reads capabilities, instance, authoritative state, and execution status, and emits one stable summary envelope without mutation.
- `cflx control enqueue <change-id> --json` expresses the high-level intent “admit this change to the existing owner.” The client reads authoritative action/lifecycle fields and uses the smallest supported sequence of existing typed commands. It owns revision refresh, idempotency keys, command-record settlement, bounded stale-revision retries, and mode-aware choice of execution mark, queue intent, retry, and start. It never starts a second owner.
- `cflx control wait <change-id> --json [--timeout <duration>]` observes the same owner until repository-verifiable success or a typed unsuccessful terminal result. It does not mutate, retry, archive, merge, or repair work.

The public CLI contract is intent-based. It must not expose flags for `expected_revision`, raw command type, queue marks, or caller-provided idempotency keys. JSON output uses a versioned envelope and stable outcome/error codes; human output remains concise and diagnostics go to stderr.

`control` uses the default `${GIT_COMMON_DIR}/cflx-api.sock` and permits an explicit `--unix-socket PATH` override. A bearer token may be read only from an explicitly named environment variable; the CLI must not accept a token value in argv or print it. Missing Git identity, missing/unreachable owner, incompatible capabilities, an unbound command executor such as `cflx run`, authentication failure, owner restart during a mutation, and timeout all fail closed with distinct machine-readable outcomes.

## Acceptance Criteria

1. `cflx control` is a separate namespace and does not change `cflx run` ownership, lock, or target semantics.
2. `status --json` is read-only and reports owner instance, app mode, scheduler/activity state, and per-change authoritative status from one coherent observation.
3. `enqueue <change-id> --json` can admit an eligible idle change to a command-capable TUI owner and can add eligible work to a live owner using existing shared operator-command semantics.
4. Enqueue handles retry-eligible changes through authoritative action eligibility and refuses blocked, terminal, unknown, worktree-ineligible, or active-run-limited targets without hidden mutation.
5. Idle-owner admission never starts unrelated pre-marked changes. If Start would consume marks other than the requested change, enqueue returns a typed operator-intent conflict without clearing or starting those marks.
6. The client generates idempotency/correlation data, waits for command records to settle, refreshes stale revisions with a bounded retry policy, and never repeats a settled side effect.
7. Owner absence, owner restart, unsupported capabilities, authentication failure, unbound command execution, conflicting operator intent, and exhausted revision retries return typed non-zero errors without starting an owner or modifying repository files.
8. `wait` returns success only after current repository and owner evidence establishes the requested change's successful archive/integration outcome; rejection, unrecoverable error, process-fatal error, owner replacement, and timeout return distinct non-zero outcomes.
9. `wait` is observation-only. It never issues start, retry, queue, archive, resolve, merge, or cleanup commands.
10. JSON stdout contains exactly one versioned result envelope. Human diagnostics and errors do not contaminate JSON stdout, and secrets are never accepted in argv or emitted.
11. Feature-disabled builds reject `control` clearly and without network, lock, socket, or repository mutation.

## Explicit Completion Conditions

- CLI parsing and help expose only `status`, `enqueue`, and `wait` under `control`, plus shared connection/output options.
- A reusable internal client connects over Unix domain sockets and deserializes generated v2 DTOs rather than duplicating ad-hoc JSON field parsing.
- Integration fixtures run a real local v2 router with bound and unbound executors and prove the success, no-op, refusal, stale-revision, restart, timeout, and no-side-effect paths.
- Tests prove `status` and `wait` submit zero commands and that failed `enqueue` paths do not start a second process, acquire the repository lock, or mutate Git/workspace state.
- JSON schema/version and exit-code mappings are asserted as executable CLI behavior, including clean stdout/stderr separation.
- `cargo test --features web-monitoring --test control_cli_tests`, `cargo fmt --check`, and `cargo clippy --features web-monitoring -- -D warnings` pass.

## Scope Rationale

Socket transport, protocol shielding, intent-based enqueue, machine output, and wait semantics must ship together to provide one usable delegation boundary. Shipping only a raw API wrapper would preserve the brittle caller dependency this change exists to remove.

## Out of Scope

- Starting, supervising, or replacing a Conflux owner.
- Adding a daemon, TCP discovery, multi-project server, or remote-host control.
- Exposing arbitrary `/api/v2` requests or low-level command flags.
- Changing the existing v2 command set or its shared TUI semantics.
- Making API state, command records, or wait observations durable workflow authority.
- Automatic repair, proposal creation, acceptance override, manual archive, or merge fallback.
- `stop`, `retry`, `resolve`, or worktree-management CLI commands; add them only after demonstrated need.

## Rollout

This is additive. Existing TUI, `run`, Web UI, and direct `/api/v2` clients remain compatible. Documentation should recommend `cflx control` for agent delegation while retaining the API as the lower-level generated contract.

## Completeness Checklist

- User outcome: an agent delegates to an existing owner through three stable commands.
- Runtime wiring: CLI parser, local transport, generated DTO client, intent mapping, command settlement, and observation loop.
- Safety: no owner creation, no raw protocol flags, bounded concurrency handling, instance-change detection, no secret argv/output, and read-only wait.
- Verification: real-router integration plus process-boundary CLI assertions for success and fail-closed paths.
- Migration: none; additive command namespace.
- Non-goals: arbitrary API access and lifecycle-control expansion remain excluded.
