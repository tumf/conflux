# Design

## Why a new command rather than a retry option

Retry is admission: it classifies existing candidate evidence and dispatches. Mixing a Git write into it would make retry eligibility depend on a mutation that can fail halfway. A separate command with its own eligibility table, preview, expectations, and typed result keeps retry unchanged and makes the write observable on its own.

## Shape

```text
OperatorCommand::RefreshProposal { change_id, source_ref, expected_source, expected_head }
preview_proposal_refresh(change_id, source_ref) -> RefreshPreview   // read-only, no record
classify_refresh_proposal(display_status, blocker_kind, tracked, has_worktree, owns_managed_process) -> RefreshAdmission
```

`classify_refresh_proposal` is a pure function next to `classify_force_stop_change`, used by the API projection (`actions.refresh_proposal`), the transaction, and tests.

## Git steps

All Git calls run in the candidate worktree through the existing worktree Git backend, with the per-invocation deadlines that backend already applies:

1. `rev-parse --verify <source_ref>^{commit}` in the project repository → `source_commit`.
2. `rev-parse HEAD` in the candidate → `candidate_head`.
3. `status --porcelain=v1 --untracked-files=all` must be empty; the candidate git dir (`rev-parse --git-dir`) must contain none of `MERGE_HEAD`, `CHERRY_PICK_HEAD`, `REVERT_HEAD`, `REBASE_HEAD`, `rebase-merge`, `rebase-apply`, `BISECT_LOG`.
4. `rev-parse --verify <source_commit>:<path>` and `<candidate_head>:<path>` → blob IDs; `cat-file blob` for the source content and for the patch.
5. Write, `add -- <path>`, `commit -m …` with hooks; stream hook output with the structured context the final-commit path uses.
6. `rev-parse HEAD`, `rev-parse HEAD^`, `diff-tree --no-commit-id --name-only -r HEAD`, `status --porcelain` → classify `refreshed` / `refresh_commit_unexpected_tree` / `hook_left_workspace_dirty`.

Expectations (`expected_source`, `expected_head`) are compared as full 40-hex strings; the preview returns them in that form.

## Failure handling

Fail closed at the first violated precondition with nothing written. Resolve source refs using Git option termination; refuse symlink/nonregular source or candidate leaves and symlink path components escaping the managed worktree, with `unsafe_proposal_path`, and preserve file mode. `staged` means staging step completed, while committed/clean_after are post-operation observed Git facts, including hooks that change HEAD before failing. After the write, never clean up: a hook rejection leaves the staged file; an unexpected tree leaves the commit. The result's `effects` object is the only claim made about the worktree, and it is computed from Git reads after the fact, not from the step that was attempted.

## Transaction placement

The command enters the shared operator transaction like `force_stop_change`: eligibility and revision are validated under the gate, the Git work runs without holding the TUI event loop, and the settled result is dispatched through the ordinary command record so API, CLI, and TUI observe one outcome. The command publishes no reducer transition; the row's lifecycle fields are unchanged by design.

## Evidence invalidation

Nothing new. `collect_change_inputs` digests the candidate `proposal.md`; evidence records bind `commit_oid`/`tree_oid`; the live hold registry is keyed by fingerprint. Tests assert these consequences rather than adding a mechanism.

## Clients

- CLI: `ClientRefreshProposalArgs { change_id, source_ref, dry_run, expected_source, expected_head, json }` with clap conflicts (`--dry-run` vs expectations) and `requires` (apply needs both); operations `refresh_proposal_preview`, `control_refresh_proposal`; outcomes `previewed`, `refreshed` (plus existing `unchanged`), and `refresh_refused` → exit `28` (unused today).
- API: `CommandSpec::RefreshProposal { change_id, source_ref, expected_source, expected_head }`; `CommandResultKind::RefreshProposal {…}`; `GET /api/v2/proposals/{change_id}/refresh-preview`; `ChangeActions.refresh_proposal`; new `ActionBlockedReason` variants for the lifecycle reasons.
- TUI: `p` → `preview` → confirmation popup → apply; hint only when allowed.

## Verification strategy

- `--lib` (`candidate_proposal_refresh`): classifier table; service tests on temp repositories (real `git`, real hooks via a `pre-commit` file in the temp repository's own hooks dir); API router tests; TUI handler/render tests; evidence-invalidation tests through `build_manifest_for_workspace` and `evaluate_reuse`.
- `tests/client_cli_tests.rs` (`candidate_proposal_refresh`): isolated owner fixture plus a temp candidate repository; real binary for `refresh-proposal` and `openapi`.
- Heavy tests are `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`.
