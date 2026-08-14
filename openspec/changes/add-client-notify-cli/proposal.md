---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - src/cli.rs
  - src/client/mod.rs
  - src/client/notify.rs
  - src/client/mcp.rs
  - tests/client_cli_tests.rs
  - skills/cflx-run/SKILL.md
verifications:
  - id: client-notify-cli
    requirement: CLI users can set, inspect, and clear an execution-scoped completion callback through the existing client boundary
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Focused CLI parsing and owner integration tests prove argv-safe set/get/clear behavior and stable envelopes
    rerun: cargo test --test client_cli_tests client_notify
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add Client Notify CLI

**Change Type**: implementation

## Problem / Context

Execution-scoped completion callbacks already exist in `src/client/notify.rs` and are exposed through `cflx client mcp` as `cflx_notify_set`, `cflx_notify_get`, and `cflx_notify_clear`. A shell operator cannot use the same capability directly without speaking MCP or calling `/api/v2` manually.

The TUI remains alive after one execution finishes, so process exit is not a completion signal. The CLI needs a direct, bounded way to register and manage the existing owner-side callback.

## Proposed Solution

Add an intent-shaped `cflx client notify` command group with three subcommands:

- `cflx client notify set <change-id> <execution-id> [--instance-id <id>] [--blocked] -- <command> [args...]`
- `cflx client notify get <change-id> <execution-id> [--instance-id <id>]`
- `cflx client notify clear <change-id> <execution-id> [--instance-id <id>]`

The commands SHALL reuse `src/client/notify.rs`, existing connection resolution, stable result envelopes, output modes, transport restrictions, and owner/execution/change coherence checks. Callback input remains an argv vector and SHALL NOT accept shell source.

## Acceptance Criteria

- Shell users can set, inspect, and clear one execution-scoped completion callback without running an MCP host.
- `set` requires a non-empty command after `--`, preserves argument boundaries, and supports blocked-event opt-in.
- All three commands support concise human output and `--json` with the existing stable operation, outcome, and exit-status contract.
- Optional `--instance-id` detects owner replacement; omitting it retains the current notify module behavior.
- The CLI preserves the existing Unix-socket-only mutation restriction and does not become an orchestration owner.
- Help, repository documentation, and the embedded `cflx-run` skill use the direct CLI workflow and distinguish execution completion from TUI process exit.

## Explicit Completion Conditions

- `ClientCommands` exposes the nested notify group and routes all three operations to `client::notify::run` without duplicating sink protocol logic.
- `skills/cflx-run/SKILL.md` directs shell-capable agents to `cflx client notify set|get|clear`, while retaining MCP as the protocol-host alternative.
- Focused CLI tests cover help, parsing, argv preservation, JSON success/failure envelopes, blocked opt-in, get/clear, empty-command rejection, and owner-binding errors.
- `cargo test --test client_cli_tests client_notify` passes.
- `cflx openspec validate add-client-notify-cli --archive-gate` passes before archive.

## Out of Scope

- Changing callback delivery, retry, persistence, timeout, or security semantics.
- Adding shell parsing, `sh -c`, or command-string expansion.
- Automatically registering a callback during `enqueue`.
- Adding stop, retry, resolve, or raw `/api/v2` workflow commands to `cflx client`.
- Changing the MCP tool surface or removing MCP guidance for MCP-only hosts.
