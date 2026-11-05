## ADDED Requirements

### Requirement: Configured worktree command sessions API

V2 MUST extend the closed command set with `run_worktree_command`
(target opaque `worktree_id`, empty params), `create_session_worktree`
(empty params), and `stop_worktree_session` (target opaque `session_id`,
empty params), and MUST expose
`GET /api/v2/worktree-sessions` and `GET /api/v2/worktree-sessions/{session_id}`.
The command executed MUST be exactly the configured `worktree_command`
template with `{workspace_dir}` and `{repo_root}` expanded server-side; no
request MUST carry a command, argv, environment, path, branch, or base
commit. Launch MUST use the same runner path as the TUI: `sh -c`, `stdin`
from `/dev/null`, an owned process group, stagger admission, and the
configured retry policy inside one session.

`create_session_worktree` MUST create a worktree on a server-generated
`ws-session-<random>` branch at base `HEAD` under the workspace base
directory, MUST validate the cwd after creation, run `.wt/setup` from the
project root, validate after setup and immediately before launch, and MUST
remove the worktree through the same cleanup the TUI uses when setup fails
(forced worktree removal with teardown skipped; the `ws-session-*` branch is
retained exactly as the TUI leaves it), launching nothing and settling
`worktree_session_setup_failed` with the typed cleanup outcome. The creation
steps MUST run under the repository root mutation guard. The running-slot
and worktree-busy reservation MUST be taken before any Git effect and
released on every refusal, so a `capacity` refusal creates no worktree and
no branch.

Each session MUST have a process-local 128-bit `session_id`, a `kind`
(`existing` or `session`), the opaque `worktree_id`, a repository-relative
path, a closed `state` (`running`, `succeeded`, `failed`, `stopped`),
`attempt`, start/finish instants, exit code or signal, setup and cleanup
facts, and an output tail bounded at 65,536 bytes of `sanitize_detail`-ed
lines with a dropped-byte count. Output MUST NOT enter the log ring, the
event stream, or the snapshot. A replay of an identical idempotency key MUST
return the original record and launch nothing; a new request for a worktree
with a running session MUST be refused `worktree_session_busy`; more than 4
concurrently running sessions MUST be refused `worktree_session_busy` with
reason `capacity`. A launch command MUST be refused
`worktree_session_unavailable` when `worktree_command` is not configured or
the project is not a Git repository, and `command_executor_unbound` on a
headless owner.

`stop_worktree_session` MUST cancel the session through the runner's own
handle (`StreamingChildHandle::terminate`), whose cancellation path sends
SIGTERM to the session's process group, escalates to SIGKILL after 5
seconds, verifies the group, and admits no further attempt; it MUST NOT
signal the process group externally, because the configured retry policy
would relaunch inside the same session, and MUST settle only after the
leader is reaped and the cleanup report is read, with `terminated` and
`state_after`. The runner MUST be bound to a `RunCommandScope` owned by the
session registry, and owner shutdown MUST close that scope under a bounded
deadline so every running session is terminated and reaped and no further
attempt can spawn. The guarantee covers the processes that remain in the
session's process group; a descendant that left it (`setsid`,
daemonization) is explicitly outside the stop and shutdown guarantee. The shared worktree service MUST refuse deleting a worktree whose
session is running, for the TUI and the API alike. Sessions MUST be retained
in memory only (running never evicted, at most 32 finished), MUST NOT be
persisted, and MUST NOT influence scheduling, resume, acceptance, or archive.
No owner-generated field (path, branch, result, error detail, events, the
log ring, the snapshot, or the OpenAPI document) MUST disclose the template,
the expanded command, environment values, absolute paths, or credentials.
The output tail is the command's own stdout/stderr: it MUST be retained only
after control-sequence stripping and size bounding, MUST NOT be redacted or
claimed to be, MUST be readable only through the bearer-authenticated
session resource, and MUST never appear as an event, log entry, or snapshot
field. A configured command that prints its arguments, environment, or cwd
discloses them to every authenticated reader; that disclosure is the
operator's responsibility when configuring `worktree_command`, and the
documentation MUST say so.

Capabilities MUST advertise `worktree_sessions` (`available`, `commands`,
`routes`, `max_running`, `max_retained`, `max_output_bytes`, `stdin`,
`reasons`), decoding as unavailable when an older owner omits it; the
generated OpenAPI document MUST include the three commands, both routes, the
`worktree_session` and `worktree_session_stop` result kinds, the
`worktree_session_started`/`worktree_session_finished` event types, and the
error codes. Every existing route and command MUST remain unchanged on an
older owner, and a new command submitted to it MUST fail typed validation
before any effect. `cflx client` and `cflx client mcp` MUST gain no verb.

#### Scenario: Configured command runs in the addressed worktree with null stdin

- **GIVEN** `worktree_command` is configured and a worktree has an opaque ID
- **WHEN** an authenticated client submits `run_worktree_command` for it
- **THEN** one session starts with cwd equal to that worktree and both placeholders expanded
- **AND** the child's stdin is `/dev/null`
- **AND** the session resource publishes exit code, state, and the bounded output tail
- **AND** one `worktree_session_finished` event is published

#### Scenario: Session worktree is created the way the TUI creates it

- **GIVEN** `.wt/setup` exists in the project root
- **WHEN** a client submits `create_session_worktree`
- **THEN** a `ws-session-*` branch and registered worktree exist at base `HEAD`
- **AND** setup ran and the cwd was validated after create, after setup, and before launch
- **AND** the result names the new opaque `worktree_id`

#### Scenario: Setup failure cleans up and launches nothing

- **GIVEN** `.wt/setup` exits non-zero
- **WHEN** `create_session_worktree` runs
- **THEN** the worktree is removed by forced removal without teardown and the `ws-session-*` branch is retained
- **AND** no process is launched
- **AND** the record fails `worktree_session_setup_failed` with `cleanup.removed: true`

#### Scenario: Capacity refusal of creation leaves the repository untouched

- **GIVEN** 4 sessions are running
- **WHEN** a client submits `create_session_worktree`
- **THEN** it is refused `worktree_session_busy` with reason `capacity` before any Git effect
- **AND** no worktree or branch is created and the reservation is released

#### Scenario: Resend and concurrency never duplicate a session

- **GIVEN** a session is running for a worktree
- **WHEN** the same idempotency key is replayed and a new key is submitted
- **THEN** the replay returns the original record and launches nothing
- **AND** the new key is refused `worktree_session_busy`
- **AND** a configured retry inside the session increments `attempt` without a new `session_id`

#### Scenario: Stop terminates the whole process group

- **GIVEN** a running session whose command spawned a background child in the same process group, and a retry policy of at least one retry
- **WHEN** `stop_worktree_session` is submitted
- **THEN** both processes are gone after settlement and `terminated` is true
- **AND** the session reads `stopped` with its signal, `attempt` stays 1, and no new process was started

#### Scenario: Deletion is refused while a session runs

- **GIVEN** a worktree with a running session
- **WHEN** the API submits `delete_worktree` or the TUI confirms deletion
- **THEN** the shared service refuses with a typed ineligible reason
- **AND** nothing is torn down or removed

#### Scenario: Output is bounded and owner metadata discloses no secret

- **GIVEN** a template containing a credential and a command that prints 200 KiB with ANSI codes but never echoes its arguments, environment, or cwd
- **WHEN** the session finishes
- **THEN** the retained tail is at most 65,536 bytes with `truncated_bytes` counted and no escape bytes
- **AND** no owner-generated field, event, log entry, snapshot field, or record contains the credential, the template, or an absolute path

#### Scenario: Output tail is the command's own data

- **GIVEN** a configured command that echoes the expanded `{repo_root}`
- **WHEN** an authenticated client reads the session resource
- **THEN** the absolute path appears in `output.lines` exactly as printed, control sequences stripped and not redacted
- **AND** it appears in no owner-generated field, event, log entry, or snapshot field, and an unauthenticated read is refused

#### Scenario: Older owner stays compatible

- **GIVEN** an owner built before this surface
- **WHEN** a client decodes its capabilities and submits `run_worktree_command`
- **THEN** `worktree_sessions` decodes as unavailable
- **AND** the command is `validation_failed` with no record and no effect
