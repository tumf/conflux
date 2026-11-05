## ADDED Requirements

### Requirement: Abort-on-conflict remote merge API

V2 MUST extend the closed command set with `merge_worktree_abort_on_conflict`
(target opaque `worktree_id`, empty params, `deny_unknown_fields`, ordinary
revision fencing, idempotency, and the application gate) that runs the
shared base merge under `AbortOnConflict`. `merge_worktree` MUST remain
unchanged with `PreserveConflict`, its `merge_conflict` refusal, retained
intermediate state, and `local_or_tui_required` recovery.

A conflict predicted by the fresh targeted inspection MUST be refused as
`merge_conflict_predicted` before any Git mutation, with the conflict files
in the typed result. A real conflict MUST run `git merge --abort`, record its
exit status, and verify restoration from the repository — `HEAD` equals the
pre-merge commit, no `MERGE_HEAD` exists, and the root working directory is
clean under the read-only status policy — reporting `restored` as
`confirmed` only when all three hold, `unknown` when any verification could
not be read, and `failed` otherwise. A confirmed restoration MUST settle
`merge_conflict_aborted`; a failed or unknown restoration MUST settle
`merge_abort_failed` with `recovery: local_or_tui_required`, MUST NOT claim
that state was preserved, and MUST leave later incompatible mutations
refused as `root_busy` while `MERGE_HEAD` remains. The owner MUST NOT retry
the abort or run any further compensation. `on_merged` MUST run only after a
completed merge, exactly once. After a confirmed abort a following merge of
a mergeable branch MUST be admissible.

The result kind `merge_abort` MUST carry `outcome`, `premerge_head`,
`conflict_files`, `abort` (`exit_ok`, `head_after`, `merge_head_present`,
`clean_after`, `restored`, `detail`), and `recovery`, and MUST be present on
success and on every refusal after eligibility. The same verified report
MUST reach the TUI merge path: the operator-facing text MUST state that the
merge was aborted and the base restored only when restoration was confirmed,
and MUST state that the abort failed or restoration is unconfirmed
otherwise. Capabilities MUST advertise `worktrees.merge_abort` (`available`,
`command`, `predicted_conflict_refused`, `restore_verification`, `reasons`),
MUST keep `merge_conflict_preserves_state: true` and
`merge_conflict_recovery` with descriptions narrowed to `merge_worktree`,
and MUST decode `merge_abort` as unavailable when an older owner omits it.
The generated OpenAPI document MUST include the command variant, the result
kind, and the three error codes. `cflx client` and `cflx client mcp` MUST
gain no verb.

#### Scenario: Clean branch merges and runs on_merged once

- **GIVEN** a worktree branch that merges cleanly into base
- **WHEN** `merge_worktree_abort_on_conflict` is submitted
- **THEN** base contains a `--no-ff` merge commit and `on_merged` ran exactly once
- **AND** the record settles `succeeded` with `outcome: merged`

#### Scenario: Predicted conflict is refused before any mutation

- **GIVEN** the fresh inspection reports conflict files for the branch
- **WHEN** the abort-on-conflict command is submitted
- **THEN** it fails `merge_conflict_predicted` naming those files
- **AND** base `HEAD` is unchanged and no `MERGE_HEAD` exists

#### Scenario: Real conflict is aborted and restoration is verified

- **GIVEN** a merge that conflicts at execution time
- **WHEN** the owner runs `git merge --abort` and it succeeds
- **THEN** `HEAD` equals `premerge_head`, `MERGE_HEAD` is absent, the root is clean
- **AND** the record fails `merge_conflict_aborted` with `restored: confirmed`
- **AND** `on_merged` did not run and a following clean merge succeeds

#### Scenario: Failed abort is reported, not assumed

- **GIVEN** `git merge --abort` exits non-zero
- **WHEN** the owner verifies restoration
- **THEN** the record fails `merge_abort_failed` with `exit_ok: false`, `restored: failed`, and `recovery: local_or_tui_required`
- **AND** the TUI text says the abort failed rather than "Merge aborted"
- **AND** a later incompatible mutation is `root_busy` until local recovery

#### Scenario: Default merge and older owners are unchanged

- **GIVEN** an older owner and a current owner
- **WHEN** `merge_worktree` conflicts on either, and `merge_worktree_abort_on_conflict` is submitted to the older owner
- **THEN** `merge_worktree` fails `merge_conflict` and retains the intermediate state on both
- **AND** the new command is `validation_failed` on the older owner with no effect
- **AND** `merge_conflict_preserves_state` is `true` on both and `merge_abort` decodes as unavailable from the older owner
