---
change_type: implementation
priority: high
dependencies: []
references:
  - examples/integrations/opencode-auto-resume/lib/loopback.mjs
  - examples/integrations/opencode-auto-resume/callback/cflx-resume-session.mjs
  - examples/integrations/opencode-auto-resume/plugin/cflx-auto-resume.mjs
  - tests/opencode_auto_resume_example.rs
verifications:
  - id: opencode-local-boundary
    requirement: The OpenCode callback remains confined to literal loopback and owner-private state while the plugin accepts only the current enqueue envelope
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/opencode_auto_resume_example.rs
    evidence: Node-backed integration tests reject localhost and unsafe state directories, accept literal loopback, and reject malformed or incompatible enqueue envelopes
    rerun: cargo test --test opencode_auto_resume_example
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Harden the OpenCode callback local trust boundary

**Change Type**: implementation

## Problem / Context

The reference OpenCode auto-resume integration has two local trust-boundary gaps. It accepts `localhost` by spelling without proving its resolved address, and its default predictable temporary state directory is not checked for symlinks, ownership, or owner-only permissions. A local attacker can therefore redirect the callback off loopback or pre-create state that suppresses or interferes with delivery. The plugin also describes its enqueue result as versioned but does not validate the schema, outcome, or binding field types.

## Proposed Solution

- Accept only literal `127.0.0.1` and `[::1]` HTTP endpoints for the callback and plugin. Reject `localhost` and all hostnames before connecting.
- Replace the shared predictable default state path with an owner-private state location, or fail closed unless an explicitly supplied state directory is a real directory owned by the current user, not a symlink, and has mode `0700`.
- Validate the enqueue envelope's supported `schema_version`, successful admitted/already-admitted outcome, and non-empty string binding IDs before registering a sink.
- Add deterministic tests for rejection before HTTP or filesystem side effects and for the valid local path.

## Acceptance Criteria

- A `localhost` destination is rejected before any connection, while literal IPv4 and IPv6 loopback endpoints remain supported.
- A symlink, foreign-owned path where testable, non-directory, or group/world-accessible callback state path is rejected before claim or marker creation.
- The default state path cannot be pre-created by another local user to control callback claims or successful-delivery markers.
- Unsupported schema versions, wrong outcomes, and non-string or empty binding IDs do not register a completion sink.
- Existing completion, owner-restart fallback, redirect refusal, dedupe, retry, and automation-marker behavior remains intact.

## Explicit Completion Conditions

- `examples/integrations/opencode-auto-resume/` implements the literal-loopback, private-state, and envelope-validation boundaries.
- `tests/opencode_auto_resume_example.rs` contains real-socket and real-filesystem regression tests that fail without each boundary.
- `cargo test --test opencode_auto_resume_example` passes with no unexpected skips.
- Strict and archive-gate OpenSpec validation pass.

## Out of Scope

- Remote OpenCode servers or HTTPS callback destinations.
- Core MCP tool or completion-sink protocol changes.
- Multi-user shared callback state.
- Push, publication, or release automation.
