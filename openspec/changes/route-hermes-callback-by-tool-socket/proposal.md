---
change_type: implementation
priority: high
dependencies: []
references:
  - src/client
  - examples/integrations/hermes-auto-resume/__init__.py
  - examples/integrations/hermes-auto-resume/README.md
  - tests/hermes_auto_resume_example.rs
  - openspec/specs/external-lifecycle-integrations/spec.md
verifications:
  - id: project-scoped-client-routing
    requirement: MCP and Hermes callback registration resolve the correct owner from each call's project directory
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Client and heavy integration tests exercise two independent repositories, Git worktrees, conflicting inputs, and legacy low-level socket routing
    rerun: cargo test --features heavy-tests --test hermes_auto_resume_example && cargo test client
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Route Conflux MCP and Hermes callbacks by project directory

Change Type: implementation

## Problem / Context

Conflux's stdio MCP server is currently started with one working directory or a fixed `--unix-socket`. Each tool may override `unix_socket`, but that exposes a transport implementation detail to the model and still requires callers to discover `.git/cflx-api.sock` correctly. A globally configured Hermes MCP server therefore covers only one project unless every call manually supplies a socket path.

The Hermes auto-resume plugin has the same coupling. Its post-tool hook currently discards tool arguments and registers the completion sink through process-global `CFLX_UNIX_SOCKET`, so an enqueue sent to another owner can register its callback against the wrong project.

The stable user-facing identity is the project directory, not the ephemeral owner socket. Conflux already derives the default socket from the repository's Git common directory when run inside that repository; the MCP boundary must expose that repository-aware resolution per call.

## Proposed Solution

Add an optional `project_dir` connection selector to every Conflux client MCP tool and to the shared client connection boundary. For each call, Conflux resolves the supplied directory as a Git worktree/repository, obtains its absolute Git common directory, and connects to `<git-common-dir>/cflx-api.sock`.

Retain `unix_socket` as an explicit low-level override for diagnostics, tests, and non-repository transports. `project_dir` and `unix_socket` are mutually exclusive in one request; supplying both returns a typed validation refusal before any owner is contacted. If neither is supplied, existing current-working-directory resolution remains unchanged.

The Hermes post-tool hook will preserve the qualifying enqueue call's connection selector. It will register `notify set` with the same call-scoped `project_dir`, or with the same call-scoped `unix_socket` when the low-level override was used. Process-global `CFLX_UNIX_SOCKET` remains only a compatibility fallback for hosts that do not expose post-tool arguments. Call-scoped routing always takes precedence over environment fallback.

The Hermes setup documentation will register one global MCP server without a fixed project socket. Agent calls identify the target using `project_dir`.

## Acceptance Criteria

- One MCP server process can operate two independent Conflux projects by passing a different `project_dir` on each call.
- A linked Git worktree resolves through its Git common directory and reaches the same owner socket as the main worktree.
- `project_dir` is available consistently on status, enqueue, wait, notify-set, notify-get, and notify-clear tools.
- `project_dir` and `unix_socket` together produce a typed validation refusal before network or socket I/O.
- `unix_socket` remains usable by itself as a low-level override.
- Omitting both selectors preserves current-working-directory behavior.
- The Hermes hook registers the completion sink using the exact selector from the admitted enqueue call and does not retain mutable cross-call routing state.
- A malformed or unavailable project route does not replace the MCP tool result, fail the Hermes turn, or register against an environment-selected different owner.
- Documentation no longer recommends fixing a project socket in global Hermes MCP configuration.

## Explicit Completion Conditions

- The shared client route resolver accepts mutually exclusive `project_dir` and `unix_socket` selectors and is used by all six MCP tools.
- Project resolution handles ordinary repositories and linked worktrees using the absolute Git common directory.
- MCP schemas and tool descriptions present `project_dir` as the normal selector and `unix_socket` as the low-level override.
- The Hermes post-tool hook no longer discards `args`; callback registration preserves the enqueue call's selector.
- Tests prove two-project routing, worktree routing, conflict refusal with no contact, low-level socket routing, default-CWD compatibility, and plugin callback affinity.
- README examples show one socket-unbound Hermes MCP registration and per-call `project_dir` usage.
- Targeted tests, formatting, Clippy, strict/evidence validation, and archive-gate validation pass.

## Safety and Compatibility

- Project resolution performs no repository mutation and starts no owner.
- Resolution must reject a path that is not a usable Git repository/worktree rather than guessing a socket.
- Error envelopes and diagnostics may identify the rejected path but must not include credentials or environment values.
- Existing clients using only `unix_socket`, or relying on MCP server current working directory, remain supported.
- No project-to-socket registry or other out-of-worktree durable routing state is introduced.

## Out of Scope

- Reintroducing a multi-project Conflux server or project registry.
- Discovering a project from `change_id`.
- Starting a Conflux owner when no owner socket exists.
- Remote/TCP owner discovery.
- Changing workflow routing, acceptance, archive, merge, or terminal classification.
