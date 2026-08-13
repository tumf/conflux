---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/changes/archive/2026-08-13-add-cflx-client-mcp
verifications:
  - id: opencode-callback-hardening
    requirement: The example callback reaches loopback only and deduplicates concurrent successful delivery without suppressing recovery after failure
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Node-backed integration tests reject absolute paths and redirects, serialize claims, and permit retry after failed POST
    rerun: cargo test --test opencode_auto_resume_example
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Secure the OpenCode auto-resume callback

**Change Type**: implementation

## Problem / Context

The reference OpenCode auto-resume callback validates only its configured base URL. An absolute `--path` replaces that base, and default redirect following can leave loopback. Its dedupe marker uses a check-then-write race and is created before successful POST, allowing duplicate concurrent delivery while suppressing later recovery from a failed delivery.

## Proposed Solution

- Validate the base as loopback HTTP, resolve the final target, and require the resolved URL to retain the validated base's origin.
- Reject absolute, protocol-relative, and backslash-variant origin changes; disable redirect following and treat redirects as failure.
- Use an atomic filesystem claim for one in-flight delivery.
- Distinguish in-flight claim from successful-delivery marker so failure removes/releases the claim and a later execution may retry.
- Permit deterministic takeover of a claim older than a five-minute stale bound so a crashed callback cannot suppress delivery forever.
- Preserve no-secret argv and payload behavior.

## Acceptance Criteria

- Absolute `--path`, protocol-relative and backslash origin changes, non-loopback or cross-origin final URLs, and HTTP redirects cannot cause an external request.
- Two concurrent callbacks for the same event produce at most one OpenCode POST.
- A failed POST does not create a successful-delivery marker and a later invocation can retry.
- A successful POST creates durable local dedupe evidence and later duplicate invocation does not POST again.
- A fresh in-flight claim returns a distinct non-success outcome; an existing success marker returns success without another POST.
- A stale in-flight claim can be atomically taken over. Normal operation is at-most-once; a crash after POST but before success-marker promotion may produce one later redelivery and is explicitly at-least-once.

## Explicit Completion Conditions

- Node-backed tests exercise redirect refusal, absolute-path refusal, concurrent claim, failed-POST retry, and successful dedupe.
- `cargo test --test opencode_auto_resume_example` passes.
- Strict and archive-gate OpenSpec validation pass.

## Out of Scope

- OpenCode server authentication or remote OpenCode operation.
- Retry loops inside one callback invocation.
- Core owner/MCP implementation changes, handled by `harden-client-mcp-completion-sinks`.
