---
change_type: implementation
priority: high
dependencies: []
references:
  - examples/integrations/opencode-auto-resume
  - hermes send CLI
verifications:
  - id: hermes-auto-resume-example
    requirement: The reference Hermes integration registers one execution-scoped callback and returns its event to the originating Hermes messaging thread through `hermes send`.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Repository tests exercise binding validation, fixed callback argv registration, scrubbed-environment reconstruction, marked thread delivery, and delivery failure without real credentials.
    rerun: cargo test --test hermes_auto_resume_example
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add Hermes auto-resume reference integration

**Change Type**: implementation

## Problem / Context

The existing `examples/integrations/opencode-auto-resume` example binds an admitted Conflux execution to the OpenCode session that requested it. On a Hermes messaging gateway the durable return address is the originating platform/chat/thread, and `hermes send` is the supported one-shot delivery adapter for that address. Conflux does not provide a runnable reference integration that captures that routing context and registers it as an execution-scoped callback.

Without that integration, a Hermes agent must remain alive in `cflx client wait`, poll repeatedly, or rely on prompt compliance to construct a callback. Hermes gateway turns may be terminated before a long Conflux execution completes, so those paths are not durable continuation.

## Proposed Solution

Add an optional `examples/integrations/hermes-auto-resume` reference integration consisting of:

- a Hermes `post_tool_call` plugin that reacts only to the Conflux enqueue tool, validates its versioned admitted envelope, captures the request-scoped messaging platform/chat/thread, and registers one execution-scoped completion callback;
- a bounded Python callback that validates the typed Conflux event, restores only explicitly configured `HOME`, `PATH`, and `HERMES_HOME`, and invokes an absolute `hermes send --quiet --to <platform:chat:thread>` argv;
- setup and security documentation covering plugin enablement, fixed-argv registration, callback environment scrubbing, pre-registration delivery testing, readback, and the distinction between notification delivery and repository-certified success.

The integration remains reference material outside the `cflx` crate and bundled skill installation.

## Acceptance Criteria

- A successful `cflx_enqueue` / namespaced `*_cflx_enqueue` tool result with a supported schema and admitted outcome automatically registers one callback for its exact `(instance_id, execution_id, change_id)` binding.
- Unsupported, malformed, unsuccessful, or non-admitted tool results register nothing.
- The callback destination is derived only from Hermes request-scoped platform/chat/thread bindings, is preserved as fixed argv, and never comes from Conflux event contents.
- The callback invokes an absolute Hermes executable with `send --quiet --to <target>` under explicit `HOME`, `PATH`, and `HERMES_HOME`; it does not use the API Server, webhook, a shell command string, polling, or a watcher.
- The generated message has an explicit automation marker and the exact execution binding, and asks the receiving Hermes thread to verify typed outcome and repository evidence.
- Repository-local tests prove registration and delivery behavior without a running Hermes gateway, a live Conflux owner, or real credentials.

## Explicit Completion Conditions

- `examples/integrations/hermes-auto-resume/` contains the plugin, callback, shared helpers, manifest, and setup documentation.
- `tests/hermes_auto_resume_example.rs` executes the reference code against local fixtures and a fake Hermes executable and covers success plus fail-closed cases.
- `cargo test --test hermes_auto_resume_example` passes.
- `cflx openspec validate add-hermes-auto-resume --archive-gate` passes before archive.
- The change is archived and merged with no modification to Hermes user configuration or secret files.

## Out of Scope

- Modifying Hermes Agent core.
- Automatically enabling the plugin or changing a user's Hermes profile.
- Persisting Conflux workflow authority outside the repository.
- Guaranteeing delivery after the configured messaging adapter returns failure.
- Treating an automation callback as proof that the Conflux change succeeded.
- Installing the reference integration through `cflx install-skills` or packaging it in the crate.
