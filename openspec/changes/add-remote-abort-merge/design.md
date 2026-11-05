# Design: abort-on-conflict remote merge with verified restoration

## Source evidence at develop `62d256107`

- `merge_branch` (`src/vcs/git/commands/merge.rs:350-388`): clean check,
  `git merge --no-ff --no-edit <branch>`, on `CONFLICT` in stderr runs
  `let _ = run_git(&["merge", "--abort"], cwd)` and returns
  `Err("Merge conflict detected. Merge aborted. Files: …")`. The abort result
  is discarded; stdout is not scanned (the preserving variant scans both).
- `merge_branch_preserving_conflict` (`merge.rs:401-455`): same merge, reads
  conflict files from the index (`get_conflict_files`) with a stderr
  fallback, returns `PreservedMergeOutcome::Conflict { files }`.
- Backend (`src/worktree_ops/git_backend.rs:168-193`): `AbortOnConflict` →
  `merge_branch` → `Err(Internal(text))`; `PreserveConflict` →
  `MergeAttempt::Conflict { files }`.
- Service (`src/worktree_ops/service.rs:1204-1300`): root guard → locate →
  `classify_merge_eligibility(facts, policy)` (predicted conflict refused
  only for `AbortOnConflict`, as `Ineligible("the worktree conflicts with base")`)
  → `MergeStarted` → `merge_into_base` → on `Conflict` → `MergeConflict`
  error, no abort, no `on_merged` → on `Merged` → `on_merged` → `MergeCompleted`.
- TUI (`src/tui/command_handlers.rs:750-790`): submits `AbortOnConflict`,
  logs `Merge failed for '<branch>': <error>` for refusals.
- Remote port (`src/web/remote_control_api/worktrees.rs:591-608`):
  `PreserveConflict`, `map_worktree_error` → `merge_conflict`.
- Capability (`worktrees.rs:372-391`): `merge_conflict_recovery`,
  `merge_conflict_preserves_state: true`.
- Read-only status policy for the clean check:
  `is_working_directory_clean` uses `read_only_status_argv(PORCELAIN_STATUS_ARGS)`
  (`src/vcs/git/commands/basic.rs:283-286`).

## Contract

### VCS layer: typed aborting merge

`src/vcs/git/commands/merge.rs` gains:

```rust
pub enum AbortingMergeOutcome {
    Merged,
    Conflict { files: Vec<String>, abort: AbortReport },
}
pub struct AbortReport {
    pub premerge_head: String,            // rev-parse HEAD before `git merge`
    pub exit_ok: bool,                    // `git merge --abort` exit status
    pub head_after: Option<String>,       // rev-parse HEAD after abort; None if unreadable
    pub merge_head_present: Option<bool>, // `rev-parse -q --verify MERGE_HEAD`; None if unreadable
    pub clean_after: Option<bool>,        // is_working_directory_clean; None on error
    pub restored: Restoration,            // Confirmed | Failed | Unknown
    pub detail: String,                   // bounded, sanitized stderr tail of the abort
}
pub enum Restoration { Confirmed, Failed, Unknown }
pub async fn merge_branch_aborting_on_conflict(cwd, branch) -> VcsResult<AbortingMergeOutcome>
```

Steps: clean check (unchanged error text) → `premerge_head` → merge with
`stdin(Stdio::null())` → on success `Merged` → if stdout+stderr contains
`CONFLICT` or `Automatic merge failed`: files from the index with the stderr
fallback (same as the preserving variant) → `git merge --abort` → three
verifications → `restored` is `Confirmed` iff `exit_ok && head_after == Some(premerge_head) && merge_head_present == Some(false) && clean_after == Some(true)`;
`Unknown` iff any verification is `None`; otherwise `Failed`. Any other
merge failure stays `Err("git merge … failed: …")`.

`merge_branch` keeps its signature and clean-check behavior and becomes a
wrapper: `Merged` → `Ok(())`; `Conflict { files, abort }` →
`Err(VcsError::git_command(abort_message(&files, &abort)))` where the message
is `Merge conflict detected. Merge aborted; base restored to <premerge_head>. Files: …`
for `Confirmed`, `Merge conflict detected. Merge abort FAILED (…); base may still hold merge state. Files: …`
for `Failed`, and `Merge conflict detected. Merge abort attempted; restoration UNCONFIRMED (…). Files: …`
for `Unknown`. The historical sentence `Merge aborted.` appears only in the
`Confirmed` form.

### Backend and service

`MergeAttempt` gains `ConflictAborted { files, abort: AbortReport }`.
`GitWorktreeBackend::merge_into_base(.., AbortOnConflict)` maps
`AbortingMergeOutcome` onto it (no more `Internal`). `WorktreeOpError` gains:

- `PredictedConflict { files }` — Display text unchanged from today's
  `Ineligible` ("the worktree conflicts with base"), so the TUI test strings
  hold; `classify_merge_eligibility` returns it under `AbortOnConflict`.
- `MergeConflictAborted { files, abort: Box<AbortReport> }` — Display text is
  the `abort_message` above.

`merge_worktree` for `ConflictAborted`: publish `MergeFailed { error: Display }`,
return `MergeConflictAborted`, run no `on_merged`. The `Conflict` arm
(preserve) is unchanged.

### API

Command `merge_worktree_abort_on_conflict { target: WorktreeTarget, params: EmptyParams }`
(`deny_unknown_fields`, revision fencing, idempotency, gate). Port method
`WorktreeOperations::merge_abort_on_conflict(worktree_id)` on
`RemoteWorktreeOperations` calls `service.merge_worktree(path, AbortOnConflict)`
and refreshes the observation afterwards (so `root_busy`/`conflict` facts are
current).

Result kind `merge_abort` (present on success and on every refusal after
eligibility):

```json
{"kind": "merge_abort", "worktree_id": "…", "branch": "feature-x",
 "outcome": "merged" | "predicted_conflict" | "conflict_aborted" | "abort_failed" | null,
 "premerge_head": "<40 hex>" | null,
 "conflict_files": ["src/lib.rs"],
 "abort": null | {"exit_ok": true, "head_after": "<40 hex>", "merge_head_present": false,
                  "clean_after": true, "restored": "confirmed" | "failed" | "unknown", "detail": "…"},
 "recovery": null | "local_or_tui_required"}
```

Error mapping (`map_worktree_error`):

| service error | code | HTTP | meaning |
|---|---|---|---|
| `PredictedConflict` | `merge_conflict_predicted` | 409 | refused before any Git mutation |
| `MergeConflictAborted` with `restored: confirmed` | `merge_conflict_aborted` | 409 | merge failed; root proven at `premerge_head`, clean, no `MERGE_HEAD` |
| `MergeConflictAborted` with `failed` or `unknown` | `merge_abort_failed` | 409 | merge failed; restoration not proven; `recovery: local_or_tui_required`; root reports `root_busy` while `MERGE_HEAD` remains |
| `MergeConflict` (preserve, unchanged) | `merge_conflict` | 409 | unchanged |

A `merge_abort_failed` record never says "state preserved": its result shows
exactly what was observed. The owner performs no further compensation; the
Server performs none either.

### Capability and OpenAPI

`WorktreeCapabilities` gains (`#[serde(default)]`):

```json
"merge_abort": {"available": true, "command": "merge_worktree_abort_on_conflict",
  "predicted_conflict_refused": true,
  "restore_verification": ["head_equals_premerge", "no_merge_head", "root_clean"],
  "reasons": ["predicted_conflict", "conflict_aborted", "abort_failed", "restore_unknown"]}
```

`merge_conflict_preserves_state` stays `true` with description "True when a
conflicting `merge_worktree` leaves the intermediate Git state in place; the
separately advertised `merge_abort` command aborts instead." `operations`
gains `"merge_abort"`. `SUPPORTED_COMMANDS`, `ALL_ERROR_CODES`, and the
`CommandResult` enum are extended; no new route.

### TUI

`handle_merge` wording only: the `MergeFailed` event already carries the
Display text, so the popup and log show the verified sentence. No key or
flow change. `openspec/specs/tui-worktree-merge/spec.md` "Merge Error
Handling" is modified so the scenario states the verified outcome instead
of assuming the abort succeeded.

### Constitution check

No new state; restoration is read from the repository each time; nothing is
written into any worktree. The abort report is command-record evidence,
not workflow input.

## Rejected alternatives

- A `conflict_policy` parameter on `merge_worktree`: an older owner would
  refuse it safely, but `merge_conflict_preserves_state: true` would then be
  false for some calls of the same command.
- Treating a conflict as "no commit, so nothing to verify": `git merge --abort`
  can fail (index lock, hooks, unreadable objects) and leave `MERGE_HEAD`;
  claiming restoration then is exactly the defect being fixed.
- Retrying `git merge --abort` or running `git reset --hard` on failure: a
  second compensation on a repository in an unknown state is more risk, not
  less; the typed failure hands recovery to the local operator.
- Changing the preserving default or its capability sentence: breaking.

## Verification design (future implementation, not executed at proposal time)

Prefix `remote_merge_abort`. Real-Git cases use
`#[cfg_attr(not(feature = "heavy-tests"), ignore)]` with the isolated fixture
pattern of `src/orchestration/proposal_refresh/tests.rs`.

### VCS gate: `remote-merge-abort-vcs` (`src/vcs/git/commands/merge.rs` tests)

- `remote_merge_abort_clean_merge_is_merged`: `--no-ff` merge commit, no report.
- `remote_merge_abort_real_conflict_aborts_and_verifies_restoration`: two
  branches editing the same line → `Conflict`, files from the index,
  `exit_ok: true`, `head_after == premerge_head`, no `MERGE_HEAD`, clean,
  `restored: Confirmed`; `merge_branch` wrapper returns the `Merge aborted;`
  sentence.
- `remote_merge_abort_failed_abort_is_reported_not_assumed`: `PATH` shim
  `git` that fails `merge --abort` → `exit_ok: false`, `MERGE_HEAD` present,
  `restored: Failed`; wrapper text says `abort FAILED`; the shim passes every
  other invocation through to the real `git` resolved before shimming.
- `remote_merge_abort_unreadable_verification_is_unknown`: shim fails
  `rev-parse HEAD` after the abort → `restored: Unknown`, text `UNCONFIRMED`.
- `remote_merge_abort_non_conflict_failure_is_an_error`: untracked file
  collision → `Err`, no abort attempted, no `MERGE_HEAD`.

### API gate: `remote-merge-abort-api` (`src/web/remote_control_api/tests/mod.rs` and `src/worktree_ops/service/tests.rs`)

Service (fake backend):

- `remote_merge_abort_predicted_conflict_refuses_before_merge`: no `Merge`
  call, `PredictedConflict`, `MergeFailed` event, no `OnMerged`.
- `remote_merge_abort_confirmed_restoration_is_a_typed_failure` and
  `…_failed_restoration_is_distinct`: backend returns `ConflictAborted` with
  each `Restoration`; typed error variant, event text, no `OnMerged`.
- `remote_merge_abort_preserve_policy_is_unchanged`: `PreserveConflict`
  still yields `MergeConflict` with no abort call.

API, new module `src/web/remote_control_api/tests/merge_abort_tests.rs`
registered in `tests/mod.rs`:

- `remote_merge_abort_clean_branch_merges_and_runs_on_merged_once` (real Git,
  `on_merged` hook counter 1, result `outcome: merged`).
- `remote_merge_abort_predicted_conflict_is_refused_with_files` (real Git:
  fresh targeted inspection predicts; `MERGE_HEAD` absent, base `HEAD`
  unchanged, `merge_conflict_predicted`, files listed).
- `remote_merge_abort_real_conflict_settles_with_restoration_evidence`
  (service over a fake backend inside the real router: `merge_conflict_aborted`
  for confirmed, `merge_abort_failed` for failed/unknown with
  `recovery: local_or_tui_required`).
- `remote_merge_abort_failed_abort_leaves_root_busy` (real Git: conflict
  state produced by the VCS function under a failing shim; a following
  `merge_worktree` of another branch is `root_busy`; after `git merge --abort`
  run by the test — standing in for local recovery — it succeeds).
- `remote_merge_abort_next_merge_after_confirmed_abort_succeeds` (real Git:
  the VCS abort path on branch A, then the API merges clean branch B).
- `remote_merge_abort_merge_worktree_default_is_unchanged`: the existing
  preserve tests pass and a conflicting `merge_worktree` still leaves
  `MERGE_HEAD`.
- `remote_merge_abort_old_owner_capabilities_decode_as_unavailable`
  (in `dto_tests.rs`) and `…_command_is_closed` (unknown param → 422).
- TUI: `remote_merge_abort_tui_message_names_verified_outcome` in
  `src/tui/command_handlers.rs` tests over the fake backend.

### Contract gate: `remote-merge-abort-contract` (`tests/openapi_contract_tests.rs`)

- `remote_merge_abort_contract_is_published_end_to_end`: command variant,
  result kind, three error codes, `capabilities.worktrees.merge_abort`,
  `merge_conflict_preserves_state` still required and `true`; negative
  control removes the result kind and one error code from a copy.

### Non-declared regression runs the implementer must still report

`cargo test --lib --features web-monitoring remote_worktree`, the TUI
`command_handlers` merge tests, and `tests/client_cli_tests.rs` must pass
unchanged except the worktree capability `operations` assertion.

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
