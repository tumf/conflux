---
change_type: implementation
priority: high
dependencies: []
references:
  - src/client
  - examples/integrations/hermes-auto-resume/__init__.py
  - examples/integrations/hermes-auto-resume/README.md
  - tests/client_mcp_integration.rs
  - tests/hermes_auto_resume_example.rs
  - AGENTS.md
  - openspec/specs/cli/spec.md
  - openspec/specs/external-lifecycle-integrations/spec.md
verifications:
  - id: project-scoped-client-routing
    requirement: MCP and Hermes callback registration resolve the correct owner from each call's project directory
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Client and heavy integration tests exercise two independent repositories, Git worktrees, conflicting inputs, and legacy low-level socket routing
    rerun: cargo test --features heavy-tests --test hermes_auto_resume_example && cargo test --features heavy-tests --test client_mcp_integration && cargo test --lib client
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Route Conflux MCP and Hermes callbacks by project directory

Change Type: implementation

> The change slug retains the initial socket-routing name for history continuity; the reviewed public design is project-directory routing.

## Problem / Context

Conflux's stdio MCP server is currently started with one working directory or a fixed `--unix-socket`. Each tool may override `unix_socket`, but that exposes a transport implementation detail to the model and still requires callers to discover `.git/cflx-api.sock` correctly. A globally configured Hermes MCP server therefore covers only one project unless every call manually supplies a socket path.

The Hermes auto-resume plugin has the same coupling. Its post-tool hook currently discards tool arguments and registers the completion sink through process-global `CFLX_UNIX_SOCKET`, so an enqueue sent to another owner can register its callback against the wrong project.

The stable user-facing identity is the project directory, not the ephemeral owner socket. Conflux already derives the default socket from the repository's Git common directory when run inside that repository; the MCP boundary must expose that repository-aware resolution per call.

## Proposed Solution

Add an optional absolute `project_dir` connection selector to every Conflux client MCP tool, the `cflx client` CLI namespace, and the shared client connection boundary. For each call, Conflux resolves the supplied directory as a usable non-bare Git working tree, obtains both its canonical repository root and absolute Git common directory, and connects to `<git-common-dir>/cflx-api.sock`. Repository-evidence operations such as `wait` use that same selected repository root rather than the server process's current repository.

Retain `unix_socket` as an explicit low-level override for diagnostics, tests, and non-repository transports. `project_dir` and `unix_socket` are mutually exclusive only when both appear in one call; that conflict uses the existing MCP validation-error or CLI usage-error channel before owner contact and does not add a stable-envelope outcome. Either call-scoped selector overrides a namespace-level default route. If the call supplies neither, the existing namespace-level default and then current-working-directory resolution remain unchanged.

The Hermes post-tool hook will preserve the qualifying enqueue call's connection selector. It will execute `cflx client --project-dir <absolute-path> notify set ...` for a project route, or use `--unix-socket` when the low-level override was used, always passing the complete admitted owner/execution/change binding. Process-global `CFLX_UNIX_SOCKET` remains only a compatibility fallback for hosts that expose no post-tool arguments object. If arguments are available but contain no selector, the hook fails closed rather than guessing an environment route.

The Hermes setup documentation will register one global MCP server without a fixed project socket. Agent calls identify the target using `project_dir`.

## Acceptance Criteria

- One MCP server process can operate two independent Conflux projects by passing a different `project_dir` on each call.
- A linked Git worktree resolves through its Git common directory and reaches the same owner socket as the main worktree.
- `project_dir` is available consistently on status, enqueue, wait, notify-set, notify-get, and notify-clear tools.
- Two call-scoped selectors together produce the existing MCP validation error or CLI usage error before network or socket I/O; no new envelope outcome is added.
- Either call-scoped selector overrides a namespace-level default route; `unix_socket` remains usable by itself as a low-level override.
- Omitting both selectors preserves namespace-level default and current-working-directory behavior.
- `cflx_wait` derives its completion-evidence repository from the same selected project as the owner socket, even when the MCP server CWD contains a colliding change ID.
- The Hermes hook registers the completion sink using the exact selector and complete binding from the admitted enqueue call and does not retain mutable cross-call routing state.
- In the Hermes hook, a malformed or unavailable project route does not replace the original MCP tool result, fail the Hermes turn, or register against an environment-selected different owner.
- Documentation no longer recommends fixing a project socket in global Hermes MCP configuration.

## Explicit Completion Conditions

- The shared client route resolver accepts one call-scoped absolute `project_dir` or `unix_socket`, lets that call override namespace defaults, and is used by the CLI plus all six MCP tools.
- Project resolution handles ordinary repositories, linked worktrees, submodules, canonicalized symlinks, and directories below the worktree while rejecting relative paths, bare repositories, and non-repositories.
- The selected project supplies both the owner socket and repository-evidence root; `wait` never mixes them with server-CWD evidence.
- CLI/MCP schemas and tool descriptions present `project_dir` as the normal selector and `unix_socket` as the low-level override without adding an envelope outcome.
- The Hermes post-tool hook no longer discards `args`; callback registration invokes the CLI's explicit selector and preserves `instance_id`, `execution_id`, and `change_id`.
- Tests prove two-project routing, worktree routing, wait evidence isolation for colliding change IDs, call-versus-default precedence, conflict refusal with no contact, low-level socket routing, default-CWD compatibility, and plugin callback affinity.
- README, embedded skill, and `AGENTS.md` examples show one socket-unbound Hermes MCP registration and per-call `project_dir` usage.
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
