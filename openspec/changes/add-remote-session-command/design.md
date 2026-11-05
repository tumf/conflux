# Design: configured-command worktree sessions

## Source evidence at develop `62d256107`

- `Enter` in the Worktrees view: selected worktree path, refusal while
  deleting, `worktree_command` required, placeholders expanded by
  `OrchestratorConfig::expand_worktree_command`, then
  `execute_worktree_command` (`src/tui/key_handlers.rs:395-437`).
- `+`: `should_trigger_worktree_command` (template configured and Git repo),
  `build_worktree_path(worktree_base_dir)` (`proposal-<ms>` directory),
  `generate_unique_branch_name(repo_root, "ws-session", 10)`,
  `worktree_add(repo_root, path, branch, "HEAD")`, `validate_worktree_command_cwd`
  after create, `run_worktree_setup` (failure → `worktree_remove_after_setup_failure`,
  logs, return), validate after setup, validate before launch, expand, launch
  (`src/tui/key_handlers.rs:443-640`). The `WorktreePlusRuntime` trait
  (`src/tui/key_handlers.rs:24-58`) is the existing seam the TUI tests use.
- Execution: `AiCommandRunner::execute_streaming_with_retry(command, Some(cwd), None, None)`,
  output forwarded to the terminal, exit status logged
  (`src/tui/terminal.rs:32-93`); the runner spawns `sh -c` with
  `stdin(Stdio::null())`, piped stdout/stderr, `configure_process_group`, the
  stagger slot, and the retry loop (`src/ai_command_runner.rs:1000-1060`).
- Process-group cleanup primitives: `kill_process_group_immediately`,
  `cleanup_process_group_verified`, `cleanup_process_group`
  (`src/process_manager.rs:941-1150`).
- Opaque worktree identity and display redaction:
  `WorktreeRegistry`, `repository_relative_display`
  (`src/web/remote_control_api/worktrees.rs`).
- Bounded callback output and shutdown deadlines already exist for completion
  sinks (`src/web/completion_sink.rs:65-106`); the same shape is reused.
- `sanitize_detail` strips ANSI/control characters and bounds a line at
  8 KiB (`src/events.rs:98-130`).

## Contract

### Commands

| type | target / params | effect |
|---|---|---|
| `run_worktree_command` | `target.worktree_id`, `params: {}` | launch the configured command in that worktree |
| `create_session_worktree` | `params: {}` | create `ws-session-*` worktree from base `HEAD`, setup, launch |
| `stop_worktree_session` | `session_id` | SIGTERM the session's process group, SIGKILL after 5 s, reap |

All three carry `deny_unknown_fields`, revision fencing, idempotency identity,
the application gate, and `command_executor_unbound` on a headless owner.
Validation refuses a blank `session_id` or a non-32-hex `worktree_id` as
`validation_failed`.

### Availability and eligibility

`worktree_sessions.available` is `config.get_worktree_command().is_some()`
AND the project is a Git repository AND a command executor is bound. A
launch command on an owner where it is false is refused
`worktree_session_unavailable` (HTTP 409) before any Git or process effect.

`run_worktree_command` eligibility (evaluated under the application gate):

| refusal | condition |
|---|---|
| `worktree_not_found` | the opaque ID is unknown or retired |
| `worktree_session_busy` (`reason: worktree_busy`) | a session with `state: running` is bound to that worktree path |
| `worktree_session_busy` (`reason: capacity`) | 4 sessions are already running |
| `target_ineligible` (`reason: cwd_invalid`) | `validate_worktree_command_cwd(repo_root, path)` fails |
| `root_busy` | a managed deletion is in flight for that path (`WorktreeDeleting` presentation is TUI-only; the service root guard is probed instead) |

`create_session_worktree` takes the `WorktreeService` root guard for the
creation steps (so it is `root_busy` while a merge or deletion runs) and
releases it before launch. The session worktree is *not* a change worktree:
`change_is_eligible` is not consulted, no change relation is published, and
periodic inspection treats it as `not_inspected` exactly as a TUI `+` worktree.

### Provisioner (API) versus TUI `+`

`src/worktree_ops/session.rs` defines `SessionProvisioner` over the existing
`WorktreePlusRuntime`-shaped seam (`check_git_repo`,
`generate_unique_branch_name`, `worktree_add`, `validate_worktree_command_cwd`,
`run_worktree_setup`, `worktree_remove_after_setup_failure`). Its production
implementation calls the same `crate::vcs::git::commands` functions the TUI's
`ProductionWorktreePlusRuntime` calls, in this exact order:

1. `check_git_repo` → `should_trigger_worktree_command` (else `worktree_session_unavailable`)
2. `create_dir_all(worktree_base_dir)`; `build_worktree_path(worktree_base_dir)`
3. `generate_unique_branch_name(repo_root, "ws-session", 10)`
4. `worktree_add(repo_root, path, branch, "HEAD")`
5. `validate_worktree_command_cwd` (`phase: after_create`)
6. `run_worktree_setup(repo_root, path)`; on `Err` →
   `worktree_remove_after_setup_failure(repo_root, path)` → refuse
   `worktree_session_setup_failed` with `cleanup: removed | retained { reason }`
7. `validate_worktree_command_cwd` (`phase: after_setup`)
8. `validate_worktree_command_cwd` (`phase: before_launch`)
9. `expand_worktree_command(template, path, repo_root)` → launch

A recording fake proves the order equals the TUI's `prepare_plus_worktree_command`
order (the TUI test module already records these calls). The TUI handler is
not rewired in this change; both paths call the same VCS functions, and the
order test is what keeps them from drifting.

Setup in the TUI is fatal with cleanup; `WorktreeBackend::create` for change
worktrees keeps setup *non-fatal*. Session creation therefore does not go
through `WorktreeBackend::create`; it adds `WorktreeBackend::create_without_setup`
and `WorktreeBackend::remove_after_setup_failure`, used only by the
provisioner.

### Launch and supervision

The session runner is the production `AiCommandRunner` clone the TUI uses,
with the run command scope of the owner process (so owner shutdown closes
spawn admission). Launch: `execute_streaming_with_retry(&expanded, Some(&path), Some("worktree_session"), None)`.
The returned `StreamingChildHandle` and `Receiver<OutputLine>` are owned by a
spawned supervisor task per session that:

- appends each `OutputLine` to the session's bounded ring after
  `sanitize_detail`; the ring keeps the newest 65,536 bytes of lines and
  counts `truncated_bytes` for what it dropped; a line is never split;
- records `attempt` from the runner's retry notifications (one session, many
  attempts; the session ID never changes);
- on child exit records `exit_code` or `signal`, `finished_at`, and the
  terminal `state` (`succeeded` for exit 0, `failed` otherwise, `stopped`
  when a stop or shutdown terminated it), then publishes one
  `worktree_session_finished` state event;
- never writes to any file.

`stop_worktree_session`: mark `stop_requested`, SIGTERM the process group,
wait up to 5 s, then `cleanup_process_group_verified` (SIGKILL and reap). The
command settles after reap with result `worktree_session_stop { session_id, terminated, state_after }`;
a session already finished settles `no_op` with `terminated: false`. Owner
shutdown (graceful stop, TUI quit, `run` completion) performs the same
sequence for every running session under one 10 s deadline before listeners
close; a session that cannot be proven reaped is logged with its PGID and the
process still exits (nothing durable depends on it).

### Session resource

```json
{"session_id": "…", "instance_id": "…", "kind": "existing" | "session",
 "worktree_id": "…", "path": "../ws/proposal-1730000000000", "branch": "ws-session-ab12cd34ef",
 "state": "running" | "succeeded" | "failed" | "stopped",
 "attempt": 1, "started_at": "RFC3339", "finished_at": null,
 "exit_code": null, "signal": null, "stop_requested": false,
 "setup": {"ran": true, "succeeded": true},
 "cleanup": null,
 "output": {"lines": [{"stream": "stdout", "text": "…"}], "retained_bytes": 1234, "truncated_bytes": 0}}
```

`GET /api/v2/worktree-sessions` returns `{instance_id, state_revision, sessions: [...]}`
newest first; detail returns one or `not_found`. Both are bearer-gated and
served on both listeners. Never serialized: the template, the expanded
command, environment, absolute paths, the PGID. `ProcessExecutionStatus`
gains `worktree_sessions_running: u32` (computed on read, `#[serde(default)]`).

### Result kinds, errors, events, capability

- `CommandResult::WorktreeSession { session_id, kind, worktree_id, branch, path, setup, cleanup }`
  on launch success; on `create_session_worktree` refusal after creation
  (setup failed, cwd invalid) the failed record carries the same kind with
  `session_id: null` and the cleanup outcome, as `refresh_proposal` refusals do.
- `CommandResult::WorktreeSessionStop { session_id, terminated, state_after }`.
- `ErrorCode`: `WorktreeSessionUnavailable`, `WorktreeSessionBusy`,
  `WorktreeSessionSetupFailed` (all HTTP 409).
- State events `worktree_session_started` / `worktree_session_finished`
  (payload `{session_id, worktree_id, state, exit_code}`, `change_id: null`);
  the snapshot is unchanged so the revision does not move.
- `CapabilitiesResponse.worktree_sessions` (`#[serde(default)]`):
  `{available, commands: [3], routes: [2], max_running: 4, max_retained: 32,
    max_output_bytes: 65536, stdin: "null", reasons: ["worktree_busy","capacity","cwd_invalid","setup_failed","unavailable"]}`.
- Worktree capability `operations` gains `"session"`; `WorktreeEligibility`
  is unchanged (session state is on the session resource, not the row).

### Deletion refusal

`WorktreeService::with_session_probe(Arc<dyn SessionProbe>)` where
`SessionProbe::session_running(&Path) -> bool`. `delete_worktree` consults it
under the root guard before teardown and refuses
`WorktreeOpError::Ineligible("a worktree session is running")`; the API maps
it to `target_ineligible`, the TUI shows its ordinary refusal popup.
`merge_worktree` is not gated (merging reads the branch, not the directory).

### Retention and constitution

Sessions live in `WorktreeSessionRegistry` inside `WebState`. Running sessions
are never evicted; finished ones are capped at 32 (oldest evicted). Nothing is
persisted; restart forgets everything, and no scheduling, resume, acceptance,
or archive decision reads the registry. The session worktree itself is an
intended operator deliverable, as it is for the TUI `+`.

## Rejected alternatives

- A free-form `command` or `argv` parameter: explicitly out; the API would
  become a remote shell.
- Running the command inline in the request: a request is not a terminal; the
  TUI blocks because the operator is watching it.
- Routing session output into `/api/v2/logs` or the event stream: a chatty
  command would churn the log ring and the sequence; output belongs to the
  session.
- Disabling the configured retry policy for API launches: it would make the
  API and the TUI disagree about what "run the worktree command" does.
  Idempotency and single-flight are what prevent *duplicate sessions*;
  retries happen inside one.
- Rewiring the TUI `+` handler onto the provisioner now: the TUI spec pins
  log lines and ordering; the order-equivalence test is the cheaper guarantee.

## Verification design (future implementation, not executed at proposal time)

Prefix `remote_worktree_session` for every new case. Real-Git and
real-process cases use `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`,
the isolated `TempDir` Git fixture pattern of
`src/orchestration/proposal_refresh/tests.rs`, a `worktree_command` template
pointing at a shell script written by the test, and bounded reap waits;
liveness is asserted through channels and state transitions, never sleeps.

### API gate: `remote-worktree-session-api` (`src/web/remote_control_api/tests/mod.rs`)

New module `src/web/remote_control_api/tests/worktree_session_tests.rs`
registered in `tests/mod.rs`, wired with a real router, `WebState`, a real
`WorktreeService` over a disposable repository, and a config whose
`worktree_command` is `sh <script> {workspace_dir} {repo_root}`:

- `remote_worktree_session_run_executes_in_the_worktree_with_null_stdin`:
  the script writes `pwd`, `$1`, `$2`, and `read x; echo stdin:$?` to a file
  and stdout; assert cwd equals the worktree, placeholders expanded, stdin
  read fails (null), exit code and output tail published, one
  `worktree_session_finished` event, record settled at launch.
- `remote_worktree_session_create_provisions_setup_and_launches`: with a
  `.wt/setup` in the project root that writes a marker, assert the new
  branch `ws-session-*`, the registered path under the base dir, the marker,
  the three validations (recorded by the provisioner), the opaque
  `worktree_id` in the result and in `/api/v2/worktrees`, and
  `inspection: not_inspected`.
- `remote_worktree_session_setup_failure_cleans_up_and_launches_nothing`:
  failing setup → worktree removed, branch removed, no process, failed record
  with `cleanup.removed: true`; a cleanup that itself fails reports
  `retained` and the reason.
- `remote_worktree_session_invalid_cwd_before_launch_is_refused`: remove the
  worktree between setup and launch via the fake seam; no process starts.
- `remote_worktree_session_replay_and_concurrency_launch_once`: replayed key
  → same record, launch counter 1; new key while running →
  `worktree_session_busy`; 4 running + 1 → `capacity`; a retrying script
  (fail first attempt) → `attempt: 2`, one `session_id`.
- `remote_worktree_session_stop_kills_the_process_group_and_reaps`: script
  `sleep 300 & sleep 300` → stop → both gone (PGID probe through `kill -0`
  on the recorded PIDs), `terminated: true`, state `stopped`, signal set.
- `remote_worktree_session_shutdown_terminates_running_sessions`: runtime
  shutdown with one running session → reaped within the deadline.
- `remote_worktree_session_output_is_bounded_and_sanitized`: 200 KiB of
  output with ANSI → tail ≤ 64 KiB, `truncated_bytes > 0`, no escape bytes.
- `remote_worktree_session_discloses_no_command_path_or_secret`: template
  containing `--token SECRET` → no response, event, or record contains
  `SECRET`, the template, or an absolute path.
- `remote_worktree_session_retention_evicts_finished_only`: 33 finished →
  32 retained; a running one is never evicted.
- `remote_worktree_session_unavailable_without_worktree_command`: no template
  → capability `available: false`, command `worktree_session_unavailable`,
  no Git mutation.
- `remote_worktree_session_delete_is_refused_while_running` (API half).
- `remote_worktree_session_old_owner_capabilities_decode_as_unavailable`
  (in `dto_tests.rs`), and `remote_worktree_session_commands_are_closed`
  (unknown fields, blank IDs → 422, no record).

### Service gate: `remote-worktree-session-service` (`src/worktree_ops/service/tests.rs`)

- `remote_worktree_session_delete_refuses_with_a_running_session`: fake probe
  true → `Ineligible` before teardown, no backend mutation call; probe false →
  existing delete path unchanged.
- `remote_worktree_session_provisioner_order_matches_tui_plus`: the
  provisioner over a recording fake performs exactly the TUI's call sequence
  (also asserted in `src/tui/key_handlers.rs` tests against the same expected
  list), including the cleanup call on setup failure.

### Contract gate: `remote-worktree-session-contract` (`tests/openapi_contract_tests.rs`)

- `remote_worktree_session_contract_is_published_end_to_end`: three command
  variants, two routes, two result kinds, three error codes,
  `capabilities.worktree_sessions`, `worktree_sessions_running`, DTO values
  constructed field by field validate; negative control removes one route
  and one result kind from a copy and must fail.

### Non-declared regression runs the implementer must still report

`cargo test --lib --features web-monitoring remote_worktree`, the TUI
`key_handlers` and `worktrees` test modules, `tests/process_cleanup_test.rs`,
and `tests/client_cli_tests.rs` must pass unchanged except for the worktree
capability `operations` assertion gaining `"session"`.

## Independent review launch contract (for the implementation worker)

Independent implementation reviews are launched exactly as follows and never
through the built-in Agent/Task tools or a direct `claude` session:

```text
PATH=/home/tumf/.local/bin:/home/tumf/.cargo/bin:/home/tumf/bin:/usr/local/bin:/usr/bin:/bin
HOME=/home/tumf
/home/tumf/.cargo/bin/agent-exec run --until 100 --stdin-file /tmp/<unique-UUID>.md -- \
  /home/tumf/.local/bin/claude-auto-fable --output-format stream-json --verbose \
  --include-partial-messages --dangerously-skip-permissions --print
```

- No `--model` flag. The review session is separate from the implementation
  session and reads only the assigned diff plus compatibility; it is not a
  repository audit.
- The worker returns the review job ID, the raw log path, the exact argv, the
  actually responding model, and the final answer. A running job is neither
  killed nor duplicated.
- The parent owns acceptance; a wrapper or `wait` exit status `0` is not
  proof. No push, release, install, owner replacement, or global
  configuration edit is performed by the worker or the reviewer.
