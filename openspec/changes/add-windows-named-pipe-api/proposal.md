---
change_type: implementation
priority: high
dependencies: []
references:
  - src/main.rs
  - src/web/mod.rs
  - src/web/unix_socket.rs
  - src/web/remote_control_api/mod.rs
  - src/client/mod.rs
  - src/client/transport.rs
  - src/client/session.rs
  - src/client/mcp.rs
  - src/cli.rs
  - tests/client_cli_tests.rs
  - tests/client_windows_tests.rs
  - openspec/specs/cli/spec.md
  - openspec/specs/remote-control-api/spec.md
verifications:
  - id: local-transport-tests
    requirement: Platform-independent endpoint derivation and shared HTTP framing preserve the existing Unix client contract while adding the Windows route
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/client/transport.rs
    evidence: "cargo test --lib client::"
    rerun: "cargo test --lib client::"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: windows-named-pipe-tests
    requirement: A Windows owner and client exchange the real /api/v2 contract over an owner-only local Named Pipe, proven by a committed native-run evidence artifact
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/client_windows_tests.rs
    evidence: "openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log (committed transcript from hx90 containing hostname, rustc -vV, tested commit hash, and a non-zero executed-test count)"
    rerun: "on hx90: cargo test --test client_windows_tests with stdout and metadata captured into openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log"
    prerequisites: []
    execution_class: physical-device
    completion_role: operational-observation
  - id: windows-native-evidence-check
    requirement: Committed Windows evidence identifies the tested commit and proves that native Named Pipe tests executed rather than cfg-skipping
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/verify_windows_native_evidence.py
    evidence: "uv run python tests/verify_windows_native_evidence.py openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log"
    rerun: "uv run python tests/verify_windows_native_evidence.py openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log"
    prerequisites:
      - windows-named-pipe-tests
    execution_class: repository-local
    completion_role: change-blocking
---

# Add Windows Named Pipe local API transport

**Change Type**: implementation

## Problem / Context

The existing owner client and local `/api/v2` server use a Unix-domain socket. The Windows compatibility branch compiles safely by returning `feature_unavailable`, but Windows cannot operate an existing owner, use proposal subscriptions, or expose the same MCP controls.

The API router, DTOs, optimistic revision checks, command registry, authentication, and completion logic are already transport-independent. Only endpoint derivation, connection establishment, listener acceptance, and trusted-local transport classification remain Unix-specific.

## Proposed Solution

Add a Windows Named Pipe transport and keep the existing HTTP/1.1 `/api/v2` wire contract unchanged.

- Unix continues using `${GIT_COMMON_DIR}/cflx-api.sock` with mode `0600`.
- Windows defaults to `\\.\pipe\cflx-v2-<sha256>`, where the digest input is the current user SID, a NUL separator, and the normalized canonical Git common-directory path. The path is canonicalized, `\\?\` is removed, separators become `\`, and Windows case-insensitive comparison is represented by lowercase normalization before hashing. Raw SIDs and repository paths are never published in the pipe name.
- The server creates the first pipe instance with `first_pipe_instance(true)`, byte mode, `reject_remote_clients(true)`, and a protected DACL granting generic-all access only to the current user SID. Security attributes are supplied at creation time; no permissive pipe is created and tightened later.
- The accept loop keeps one unconnected server instance available, creates the replacement instance before dispatching a connected stream, and stops accepting on the existing cancellation token. Each stream is served by the same Axum router and `WebState` as Unix and TCP.
- Named Pipe requests are marked as a trusted local transport. The existing subscription and completion-sink rules that permit argv storage/disclosure over the owner-only Unix socket also permit the owner-only Named Pipe. TCP remains untrusted and unchanged.
- The client derives the same default name from the selected project's Git common directory and current user SID, opens the pipe with Tokio, and sends the existing bounded HTTP/1.1 requests and event stream. Response limits, timeouts, bearer-token validation, revision coherence, outcomes, and exit statuses remain unchanged.
- `project_dir` remains the normal portable selector. `unix_socket` remains a Unix-only low-level override. No new public `named_pipe` override is added in this change; deterministic project routing is sufficient and avoids an additional platform-specific CLI/MCP surface.
- Builds without `web-monitoring` remain fail-closed with `feature_unavailable`.

The implementation may add `sha2` and the narrowly required `windows` crate feature flags. It must not add a second HTTP implementation, a Windows-only API contract, durable endpoint registry, or loopback TCP fallback.

## Change Boundary

In scope:

- platform-local endpoint derivation and transport types;
- Windows Named Pipe creation, security descriptor ownership, accept loop, connection, and shutdown;
- refactoring the current client HTTP framing so Unix and Windows streams share it;
- trusted-local transport classification for Named Pipe;
- Windows integration coverage and Unix regression coverage;
- CLI/help/operator documentation that currently states Unix is the only local client transport;
- canonical CLI and remote-control API spec deltas.

Out of scope:

- merging this branch into `main`;
- publishing a release;
- loopback TCP fallback or remote Named Pipe access;
- changing `/api/v2`, OpenAPI DTOs, command semantics, outcome names, exit statuses, MCP tools, repository locks, or completion evidence;
- adding a public arbitrary-pipe override;
- supporting Windows versions without Named Pipes.

## Acceptance Criteria

- A default Windows owner starts a Named Pipe listener without `--web`, publishes a `pipe://` endpoint, and serves the same `/api/v2` router and state as Unix.
- `cflx client status`, lifecycle controls, wait, subscriptions, and `cflx client mcp` use the Windows pipe rather than returning `feature_unavailable`.
- Main and linked worktrees for one repository derive one pipe name; distinct repositories or Windows users derive different names.
- The pipe rejects remote clients and principals other than the current user, and first-instance acquisition fails closed when the name is already owned.
- Before sending request bytes, the client verifies that the connected pipe owner SID equals the current process token user SID and otherwise returns a typed transport refusal without retry or downgrade.
- Subscription argv storage/disclosure is allowed over Unix and Named Pipe, and remains refused over TCP.
- Named Pipe request and response handling retains existing HTTP framing, response-size ceiling, timeouts, authentication validation, revision checks, typed outcomes, and stdout/stderr contracts.
- Listener startup is transactional: failure to create any requested listener stops already-started listener tasks and starts no orchestration work.
- Graceful shutdown stops new Named Pipe accepts and waits for active listener/connection tasks.
- Unix behavior and tests remain unchanged.

## Explicit Completion Conditions

- Repository tests cover deterministic Windows pipe naming, user/repository separation, trusted-local classification, shared HTTP framing, first-instance refusal, and cancellation behavior.
- `cargo test --lib client::` passes on the development host.
- On a native Windows host, `cargo test --test client_windows_tests` starts a real owner-side Named Pipe test server using the production listener and shared router, exercises at least status and one subscription refusal/permission boundary through the production client, and passes.
- The native Windows transcript is committed at `openspec/changes/add-windows-named-pipe-api/evidence/windows-native-tests.log`, records hostname, `rustc -vV`, tested commit hash, and an executed-test count greater than zero; `0 passed` never satisfies this condition.
- On a native Windows host, `cargo check --locked --all-targets` and `cargo build --release --locked` pass.
- Existing Unix client and listener integration tests pass.
- `cflx openspec validate add-windows-named-pipe-api --archive-gate` passes after implementation.

## Verification Ownership

Focused repository-local tests block completion. Native Windows execution is performed on the available Windows builder before the branch is reported complete. Repository-wide format, lint, and ordinary tests remain owned by the tracked commit hooks.

## Branch Policy

The proposal, implementation, and resulting commits remain on the `fix/windows-build-compatibility` branch lineage. They MUST NOT be merged into `main` as part of this change.
