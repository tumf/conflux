## ADDED Requirements

### Requirement: Candidate proposal refresh is one explicit target-scoped operator command

The shared operator service MUST offer one explicit **refresh proposal** command with a read-only preview form and an apply form, reachable from the typed remote API, the CLI client, and the TUI through the same process-local transaction. Both forms require an explicit `source_ref`; there is no default source and no hardcoded branch name. The command MUST read only the blob at `openspec/changes/<change_id>/proposal.md` from the resolved source commit and MUST NOT merge, rebase, fetch, or read any other path.

Eligibility MUST be a pure function of authoritative owner facts, published per change as `actions.refresh_proposal`: the target is tracked, has a registered managed worktree, owns no live managed process, and its display status is `error`, `stopped`, `stalled`, `blocked` with blocker kind `external`, or `not queued`. Other rows MUST be refused with the stable reasons `unknown_target`, `no_candidate_worktree`, `terminal_target`, `target_active`, `target_admitted`, `merge_wait`, `resolve_wait`, or `reject_wait`, independent of process mode.

Apply MUST carry `expected_source` and `expected_head` as full-length commit IDs obtained from a preview and MUST refuse `refresh_expectation_mismatch` when either differs from the current resolved source commit or candidate `HEAD`. Before writing, the candidate MUST be clean under full porcelain status with untracked files and no Conflux-owned path excluded (`candidate_dirty`) and MUST have no in-progress Git operation (`candidate_operation_in_progress`); an unresolvable ref, a missing source blob, or a missing candidate blob MUST be refused as `source_ref_unresolvable`, `source_proposal_missing`, or `candidate_proposal_missing`. Identical blobs MUST settle as `unchanged` with no write.

The write MUST be exactly: write the source blob to the candidate path, stage that path only, and create one commit with repository hooks enabled (never `--no-verify`) on top of `expected_head`, streaming hook output with structured context. The result MUST be `refreshed` only when `HEAD` advanced by exactly one commit whose sole parent is `expected_head`, whose tree change is exactly that path, and the worktree is clean afterwards. A hook exiting non-zero MUST settle as `hook_rejected` with the written file left staged and nothing reset; a Git commit failure as `commit_failed`; a commit touching other paths as `refresh_commit_unexpected_tree`; a dirty worktree after commit as `hook_left_workspace_dirty`; in every case `HEAD` and the index are left as the failure found them, and the result reports `effects {file_written, staged, committed, clean_after}` computed from Git reads, plus `source_commit`, `source_blob`, `candidate_head_before`, `candidate_head_after`, `candidate_blob_before`, and `path`.

The command MUST NOT start, retry, queue, mark, unmark, dequeue, stop, resolve, archive, or merge; MUST NOT change the row's display status, queue intent, execution mark, blocker, terminal evidence, attention, or retry eligibility; MUST NOT touch the source branch, unrelated changes, or other worktrees; MUST NOT persist refresh history as workflow authority; and MUST NOT add configuration or policy toggles. Resume routing after a refresh MUST remain derived from candidate files and Git on the next explicit retry, and stale Acceptance evidence MUST be invalidated by the existing content identity (manifest change inputs and commit/tree bindings) rather than by any new state.

Source refs MUST use option-terminating Git argument handling. Nonregular or symlink source/candidate entries and candidate path traversal through symlinks MUST be refused before writes as `unsafe_proposal_path`; file mode is preserved. `effects.staged` means the staging step completed, not current index dirtiness; `committed` and `clean_after` MUST reflect actual post-hook Git reads even when a hook exits nonzero or advances HEAD unexpectedly. Pre-write refusals are no-effect; post-write/hook failures retain actual recoverable state and never promise unchanged HEAD/index contrary to observation. A repository read that cannot be completed MUST be refused as `git_read_failed` rather than guessed, and a failed write of the candidate file as `write_failed`; both report effects re-read from Git like every other refusal.

#### Scenario: Preview reports facts and mutates nothing

- **GIVEN** an eligible `error` candidate whose `proposal.md` differs from the one at the named source commit
- **WHEN** the preview form runs with that `source_ref`
- **THEN** it reports the resolved `source_commit`, `candidate_head`, `path`, both blob identities, `no_op: false`, and a unified patch
- **AND** no command record, intent, Git write, mark, queue, or revision effect occurs

#### Scenario: Apply creates one scoped hook-retaining commit

- **GIVEN** a preview reported `source_commit` S and `candidate_head` H for an eligible clean candidate with applied implementation commits
- **WHEN** apply runs with `expected_source: S` and `expected_head: H`
- **THEN** the candidate `HEAD` becomes a new commit whose sole parent is H and whose only tree change is `openspec/changes/<id>/proposal.md`
- **AND** repository hooks ran and their output was streamed
- **AND** the base branch tip, implementation files, and every other candidate file are unchanged
- **AND** the result is `refreshed` with `effects {file_written: true, staged: true, committed: true, clean_after: true}`

#### Scenario: Identical content is a truthful no-op

- **GIVEN** the candidate and source blobs are identical
- **WHEN** apply runs with matching expectations
- **THEN** the result is `unchanged`
- **AND** no file is written, staged, or committed

#### Scenario: Expectation, cleanliness, and operation preconditions refuse without writing

- **GIVEN** apply is submitted with a stale `expected_head`, or against a candidate with a staged, unstaged, or untracked entry, or during an in-progress merge
- **WHEN** the shared transaction evaluates it
- **THEN** it refuses with `refresh_expectation_mismatch`, `candidate_dirty`, or `candidate_operation_in_progress`
- **AND** `HEAD`, the index, the worktree, marks, queue intent, mode, hooks, and scheduler state are unchanged

#### Scenario: Hook rejection retains recoverable staged state

- **GIVEN** the candidate's `pre-commit` hook exits non-zero
- **WHEN** apply reaches the commit step
- **THEN** the result is `hook_rejected` with `effects {file_written: true, staged: true, committed: false}` and the hook output tail
- **AND** the written file remains staged, `HEAD` is unchanged, and nothing is reset or cleaned

#### Scenario: Unexpected commit tree is reported, not repaired

- **GIVEN** a hook re-stages an unrelated file during the commit
- **WHEN** post-commit verification runs
- **THEN** the result is `refresh_commit_unexpected_tree` with `committed: true`
- **AND** the commit is left in place and no amend, reset, or cleanup is attempted

#### Scenario: Ineligible rows are refused by the published table

- **GIVEN** rows in `applying`, `queued`, dependency-`blocked`, `merge wait`, `archived`, and a tracked row without a managed worktree
- **WHEN** eligibility is projected and apply is submitted
- **THEN** `actions.refresh_proposal` carries `target_active`, `target_admitted`, `target_admitted`, `merge_wait`, `terminal_target`, and `no_candidate_worktree` respectively
- **AND** apply is refused with the same reason before any Git read

#### Scenario: Refresh never admits work and ordinary retry still does not refresh

- **GIVEN** a refreshed `error` candidate
- **WHEN** the command settles
- **THEN** the row remains `error`, unmarked, not queued, and no scheduler, retry, or start effect occurs
- **AND** a later explicit retry reads the corrected candidate proposal through ordinary resume routing
- **AND** an explicit retry on an un-refreshed candidate performs no refresh

#### Scenario: Stale Acceptance evidence is invalidated by content identity

- **GIVEN** a gate evidence record and an in-memory unchanged-input hold bound to the pre-refresh candidate
- **WHEN** the refresh commit lands and the Acceptance manifest is rebuilt
- **THEN** the manifest `change_input_digest` and fingerprint differ from the pre-refresh values
- **AND** the record decides `Rerun` on `commit_oid`
- **AND** the old hold no longer refuses admission
