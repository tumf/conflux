---
change_type: implementation
priority: high
dependencies: []
references:
  - src/client/wait.rs
  - tests/client_cli_tests.rs
  - openspec/specs/cli/spec.md
verifications:
  - id: client-wait-unknown-change
    requirement: Unknown change IDs are refused immediately without changing known wait behavior
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/client_cli_tests.rs
    evidence: cargo test --test client_cli_tests wait_refuses_unknown_change
    rerun: cargo test --test client_cli_tests wait_refuses_unknown_change
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Refuse unknown client wait targets immediately

**Change Type**: implementation

## Problem / Context

`cflx client wait <change-id>` currently treats a change ID absent from the owner's coherent snapshot like a change that may later progress. With the default unbounded timeout, a typo such as `cflx client wait aaaa` can therefore wait indefinitely.

The client already exposes the typed `change_not_found` outcome and exit status `9` for other target-scoped operations. Wait should use the same refusal when its initial coherent observation proves that the requested change does not exist.

## Proposed Solution

On the initial coherent observation, when the requested change is absent from the owner's published proposal rows, perform one bounded repository certification of the owner's declared terminal mode. If evidence proves completion, return `completed` exactly as an observed settled row would. Otherwise classify the target as `change_not_found` and return immediately with exit status `9`, the requested `change_id`, the observed owner instance, and zero submitted commands.

Preserve the existing behavior for a known change in `not queued` or another owner-progressing state. Later disappearance remains governed by the existing rule that disappearance alone is not completion; this change does not reinterpret it as either success or a newly unknown target.

## Acceptance Criteria

- An unknown change on the initial coherent observation returns `change_not_found` immediately with exit status `9`.
- An initially absent change whose repository evidence already certifies the declared terminal mode returns `completed`, not `change_not_found`.
- The refusal identifies the requested change and owner instance and submits no mutation command.
- A known `not queued` change continues observing rather than being refused as unknown.
- A change that disappears after being observed remains subject to the existing non-completion behavior.

## Explicit Completion Conditions

- `src/client/wait.rs` distinguishes initial absence from a known waiting row and from later disappearance.
- `tests/client_cli_tests.rs` contains a regression test that fails if an unknown initial target waits until timeout or if a known `not queued` target is refused.
- `cargo test --test client_cli_tests wait_refuses_unknown_change` passes.

## Out of Scope

- Changing completion evidence or terminal-mode certification.
- Treating later snapshot disappearance as successful completion.
- Adding mutation, retry, queue, or repair behavior to `wait`.
- Changing timeout defaults or transport deadlines.
