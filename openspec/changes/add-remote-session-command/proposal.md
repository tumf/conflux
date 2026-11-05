---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/remote-worktree-operations/spec.md
  - openspec/specs/tui-worktree-view/spec.md
  - openspec/specs/process-execution/spec.md
  - docs/guides/USAGE.md
verifications:
  - id: remote-worktree-session-api
    requirement: Typed launch of the configured worktree_command in an existing worktree or a fresh ws-session worktree with owner-managed cwd, setup, cleanup, single-flight, stop, bounded sanitized output, and compatibility
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/mod.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_worktree_session -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_worktree_session -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-worktree-session-service
    requirement: Shared worktree service refuses deletion of a worktree with a running session and the session provisioner performs exactly the TUI plus-key step order
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/worktree_ops/service/tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_worktree_session -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_worktree_session -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-worktree-session-contract
    requirement: Generated OpenAPI, capabilities, error codes, and result kinds publish the session surface completely and older owners decode as unavailable
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/openapi_contract_tests.rs
    evidence: cargo test --features web-monitoring --test openapi_contract_tests remote_worktree_session -- --nocapture
    rerun: cargo test --features web-monitoring --test openapi_contract_tests remote_worktree_session -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add remote configured-command worktree sessions

**Change Type**: implementation

## Problem / Context

Conflux-server (CFSRV-51, OP-34 / OP-35) needs the two TUI Worktrees-view
actions that run the *configured* `worktree_command`: `Enter` runs it in the
selected existing worktree (`src/tui/key_handlers.rs:395-437`), and `+`
creates a change-independent `ws-session-*` worktree from base `HEAD`,
validates the cwd, runs `.wt/setup`, cleans the worktree up when setup fails,
revalidates, and then runs the command (`src/tui/key_handlers.rs:443-640`,
`openspec/specs/tui-worktree-view/spec.md`). Both run through
`AiCommandRunner::execute_streaming_with_retry` with `stdin(Stdio::null())`,
its own process group, the stagger slot, and the configured retry policy
(`src/tui/terminal.rs:32-93`, `src/ai_command_runner.rs:1024-1045`). The TUI
is *not* interactive here; an earlier inference to the contrary was disproven
by source inspection and is not adopted.

At develop `62d256107`, `/api/v2` offers `create_worktree` only for a change
ID and no way to run the configured command at all. The accepted requirement
note conditions the feature on: no shell source or argv from the API, typed
start with execution identity, state, exit code, and bounded logs; cflx-owned
cwd, branch, setup, and failure cleanup; no duplicate launch on resend or
concurrent requests, kept distinct from configured internal retries; log size
limits and secret protection; and no arbitrary path exposure.

## Proposed Solution

Add a closed, owner-managed *session* surface:

- Commands `run_worktree_command` (target opaque `worktree_id`, empty params),
  `create_session_worktree` (empty params; creates the `ws-session-*`
  worktree exactly as `+` does and then launches), and
  `stop_worktree_session` (target opaque `session_id`, empty params, the same
  `target`/`params` shape as the worktree commands). The command text is always the
  configured `worktree_command` template with `{workspace_dir}` and
  `{repo_root}` expanded server-side; no request field carries a command,
  argv, path, branch, or environment.
- Resources `GET /api/v2/worktree-sessions` and
  `GET /api/v2/worktree-sessions/{session_id}` publish each session's
  process-local 128-bit `session_id`, `kind` (`existing` or `session`), the
  opaque `worktree_id` and repository-relative display path, branch, closed
  `state` (`running`, `succeeded`, `failed`, `stopped`), attempt count, start
  and finish instants, exit code or signal, setup and cleanup facts, and a
  bounded, sanitized output tail with truncation counts. Output lines are
  never log entries, events, or snapshot fields, so a chatty command cannot
  advance the revision.
- Execution matches the TUI: `sh -c <expanded template>`, `stdin` null,
  process-group leader, stagger admission, and the configured retry policy
  inside one session (visible as `attempt`). A replay of the same idempotency
  key returns the original record and launches nothing; a second new request
  for a worktree whose session is still running is refused
  `worktree_session_busy`; at most 4 sessions run concurrently.
- Lifecycle is owner-owned: `stop_worktree_session` cancels the session
  through the runner's own handle (`StreamingChildHandle::terminate`), which
  sends SIGTERM to the process group, escalates to SIGKILL after 5 seconds,
  verifies the group, and returns without a further attempt; the command
  settles only after `wait` and the typed cleanup report. An external
  `killpg` is never used, because the configured retry policy treats any
  non-zero exit as a crash and would relaunch inside the same session. The
  session registry owns its own `RunCommandScope`; owner shutdown closes it
  (`shutdown(10 s)`), which refuses new attempts and terminates and reaps
  every running session under that bounded deadline. Descendants that left
  the session's process group (`setsid`, daemonization) are outside this
  guarantee. The shared worktree service refuses deleting a worktree whose
  session is running, for the TUI and the API alike.
- Retention is in-memory only: running sessions are never evicted; at most 32
  finished sessions are retained, oldest first; nothing is written to disk and
  nothing survives restart.
- Capabilities gain `worktree_sessions` (`available` is true only when
  `worktree_command` is configured, the project is a Git repository, and a
  command executor is bound), the generated OpenAPI publishes the three
  commands, two routes, two result kinds, and the new error codes.
  `cflx client` and `cflx client mcp` gain no verb.

## Acceptance Criteria

1. `run_worktree_command` on a live worktree starts one session whose actual
   working directory is that worktree, whose command equals the expanded
   template, whose stdin is `/dev/null`, and whose exit code and bounded
   stdout/stderr tail are published on the session resource and in a
   `worktree_session_finished` state event; the command record settles
   `succeeded` at launch with a `worktree_session` result naming the session.
2. `create_session_worktree` creates a registered worktree on a new
   `ws-session-<random>` branch at base `HEAD` under the workspace base
   directory, validates the cwd after creation, runs `.wt/setup` from the
   project root, validates again after setup and immediately before launch,
   and launches the command there; the result names the new opaque
   `worktree_id`, branch, and relative path, and the worktree list shows it.
3. When `.wt/setup` fails, the worktree is removed through the same cleanup
   the TUI uses (`worktree_remove_with_options { skip_teardown: true }`,
   i.e. `git worktree remove --force` without teardown; the `ws-session-*`
   branch is retained exactly as the TUI leaves it), no command is launched,
   and the record fails with
   `worktree_session_setup_failed` carrying the typed cleanup outcome
   (`removed` or `retained` with reason); when the cwd becomes invalid before
   launch, no process starts and the record fails with the cwd diagnostic.
4. Single-flight holds: an exact replay returns the original record without a
   second process (proven by a launch counter and a PID observation), a new
   request against a running session is `worktree_session_busy`, the fifth
   concurrent session is `worktree_session_busy` with reason `capacity`
   before any Git effect (a `capacity` refusal of `create_session_worktree`
   creates no worktree and no branch), and the configured retry policy
   retries *inside* the one session with `attempt` incremented and the same
   `session_id`.
5. `stop_worktree_session` terminates the session's whole process group,
   including a background child that stays in that group, through the
   runner's cancellation path; it settles with `terminated: true` only after
   the leader is reaped and the cleanup report is read, the session reads
   `stopped` with the signal, and with a retry policy of at least one retry
   configured no second attempt starts (`attempt` stays 1, no new PID).
   Owner shutdown does the same for every running session within the
   bounded deadline. A descendant that left the process group (`setsid`,
   daemonization) is explicitly outside the guarantee.
6. No owner-generated field (path, branch, result, error detail, events,
   the log ring, the snapshot, the OpenAPI document) contains the expanded
   command text, the configured template, any environment value, an
   absolute path, or a bearer token. The output tail is the command's own
   stdout/stderr: it is bounded at 64 KiB with dropped bytes counted and
   passed through `sanitize_detail`, which strips ANSI/control sequences and
   bounds a line at 8 KiB but redacts nothing. A configured command that
   prints its arguments, environment, or cwd discloses them to every
   bearer-authenticated reader of the session resource; choosing such a
   command is the operator's responsibility, and the tail is published
   nowhere else (never as an event, log entry, or snapshot field).
7. The shared service refuses `delete_worktree` (API, fail-closed) and the TUI
   `D`/`Y`/`S`/`X` deletion of a worktree with a running session with a typed
   ineligible reason; unrelated worktrees are unaffected.
8. Compatibility: the surface is additive. An older owner serves every
   existing route and command unchanged; a client decodes its capabilities
   with `worktree_sessions` absent as unavailable; any of the three commands
   submitted to it is `validation_failed` before any effect.

## Explicit Completion Conditions

The real-process regressions in `design.md` run under
`remote_worktree_session` with nonzero passed counts in all three declared
gates; a zero-test selection fails. The TUI `+` and `Enter` handlers keep
their log lines and behavior, and the provisioner step-order test proves the
API performs the identical sequence. `docs/guides/USAGE.md` and `AGENTS.md`
document the surface. Proposal validation is not implementation evidence;
tasks stay unchecked until implemented and verified in a separately
authorized execution.

## Scope / Preserved Contracts

Implementation touches a new `src/worktree_ops/session.rs` (provisioner and
session registry), `src/worktree_ops/service.rs` (session probe for delete),
`src/web/remote_control_api/` (DTOs, routes, executor dispatch, capability),
`src/web/openapi.rs`, `src/web/state.rs` wiring, owner shutdown in
`src/main.rs`/`src/web/mod.rs`, focused tests, and documentation. The TUI
`Enter`/`+` handlers, `AiCommandRunner`, `.wt/setup`/`.wt/teardown`
semantics, `create_worktree`, `delete_worktree` (other than the new
refusal), `merge_worktree`, revision fencing, idempotency, and authentication
are unchanged. No new dependency. No Conflux-owned file is written into any
worktree: output is retained in memory only.

The tracked `.pre-commit-config.yaml` scopes Rust checks by staged paths; the
declared focused gates stay explicit and no broad build is added as a task.

## Out of Scope / Rollout Handoff

Not included: arbitrary commands, argv, or environment from the API; an
interactive PTY or stdin relay; persisting sessions or their output; a Web
terminal UI; Server relay code; `cflx client`/MCP verbs; changing the TUI's
inline execution model. The Server passes opaque identifiers only and reads
the session resource and events. Running owners are not restarted by this
change; the feature exists only in a binary built from it.
