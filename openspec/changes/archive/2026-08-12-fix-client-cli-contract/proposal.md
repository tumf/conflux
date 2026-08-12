---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - src/main.rs
  - src/client/enqueue.rs
  - src/client/session.rs
  - src/client/transport.rs
  - src/client/wait.rs
  - tests/client_cli_tests.rs
verifications:
  - id: client-cli-contract-regressions
    requirement: cflx client preserves its JSON, deadline, authentication-header, and partial-intent audit contracts
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Focused compiled-CLI and transport tests for all four independently reviewed defects
    rerun: cargo test --features web-monitoring --test client_cli_tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix client CLI machine and safety contracts

**Change Type**: implementation

## Premise / Context

- `add-client-cli` is archived and merged, and its focused tests, OpenAPI tests, no-default-features test, fmt, clippy, strict validation, and archive gate pass independently.
- Independent review found no Critical issue but found three Major contract defects and one Minor audit defect.
- Invalid `cflx client ... --json` usage exits through Clap before the stable JSON envelope path.
- Wait operations can exceed the caller's timeout because transport and Git verification are not bounded by the remaining operation deadline.
- An environment-provided bearer token is concatenated into an HTTP header without rejecting control characters.
- A partial-intent result can claim that `set_execution_mark` was submitted when the target was already marked.

## Requested Artifact

Implementation of the four bounded corrections, with regression tests.

## Problem / Context

The client is intended as a stable delegation boundary for agents. Its machine-output and timeout contracts must hold on failure paths, and its local HTTP boundary must not accept malformed header values. Audit detail must also describe only commands actually sent. These defects do not require a new command or architecture; they require surgical corrections to the existing implementation.

## Proposed Solution

1. Parse the CLI with a non-exiting Clap path. When an invocation targets `cflx client` and requests JSON, convert usage errors to exactly one versioned `usage_error` envelope on stdout and a non-zero exit. Preserve normal Clap human diagnostics for non-JSON and non-client invocations.
2. Give `wait` one monotonic operation deadline. Bound initial/repeated observation and repository verification by the remaining duration. Propagate the remaining deadline into Git commands, including remote lookups, terminate child processes on expiry, and converge to `timeout` without mutation.
3. Validate bearer token bytes before request construction. Reject CR, LF, DEL, and other HTTP control characters without displaying the token; use a typed client error.
4. Carry the actual submitted-command list into `partial_intent` so a pre-existing mark is never reported as a submitted command.

## Acceptance Criteria

1. Invalid change IDs, timeout values, missing arguments, and unknown client flags with `--json` emit exactly one parseable versioned `usage_error` envelope on stdout and exit non-zero.
2. Human CLI parse errors remain concise and compatible, and unrelated top-level Clap errors are not rewritten as client JSON.
3. `wait --timeout D` bounds all owner observation and repository/Git verification by one monotonic deadline; a stalled socket or remote lookup cannot extend the operation materially beyond the deadline safety margin.
4. Deadline expiry returns typed `timeout`, terminates any spawned Git child used by verification, and submits no mutation.
5. Bearer tokens containing HTTP control characters fail before connecting or writing a request, and neither stream contains the token value.
6. Valid bearer tokens continue to authenticate normally.
7. `partial_intent.detail.commands_submitted` contains only commands actually submitted in this invocation, including an empty list when a pre-existing mark is followed by a failed Start.

## Explicit Completion Conditions

- Regression tests exercise the compiled CLI for JSON usage errors and human-output compatibility.
- Deterministic stalled-UDS and stalled-remote fixtures prove operation-level deadline enforcement without short timing assertions as the correctness oracle; event/process synchronization proves cancellation and no mutation.
- Transport tests cover valid tokens and CR/LF/control-character rejection before any request bytes are accepted by the fixture server.
- Enqueue tests cover both newly submitted and pre-existing marks and assert exact command audit lists.
- `cargo test --features web-monitoring --test client_cli_tests`, `cargo fmt --all -- --check`, and `cargo clippy --features web-monitoring -- -D warnings` pass.

## Out of Scope

- New client commands or outcome classes beyond wiring the existing `usage_error` and `timeout` contracts.
- Changing owner API semantics, repository completion classification, or orchestration ownership.
- Broad CLI parser refactoring unrelated to capturing client JSON usage errors.
- Changing valid token formats beyond rejecting bytes invalid at the HTTP header boundary.

## Rollout

This is a backward-compatible correction for valid invocations. Invalid JSON-mode invocations gain their promised envelope; malformed token values fail earlier and safely.
