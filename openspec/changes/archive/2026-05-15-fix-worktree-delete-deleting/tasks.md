## Implementation Tasks

- [x] Add dashboard-local delete progress state for the active worktree branch in `dashboard/src/App.tsx` (completion: `handleDeleteWorktreeConfirm` sets the branch before awaiting `deleteWorktreeAPI` and clears it in a `finally` path for both success and failure; verification: unit - dashboard component tests observe the pending state while the delete promise is unresolved).

- [x] Thread the delete progress branch through every `WorktreesPanel` render path (completion: both desktop and mobile `WorktreesPanel` usages receive the same `deletingWorktreeBranch` value, and `WorktreesPanel` forwards `isDeleting` only to the matching `WorktreeRow`; verification: unit - a `WorktreesPanel` test renders multiple worktrees and asserts only the matching branch row shows the deleting indicator).

- [x] Render and protect the deleting worktree row in `dashboard/src/components/WorktreeRow.tsx` (completion: the row displays a spinner and deleting label, disables merge/delete controls, and does not invoke `onClickWorktree` while deleting; verification: unit - `WorktreeRow` tests assert visible deleting UI and suppressed row/action callbacks when `isDeleting` is true).

- [x] Preserve existing successful and failed delete outcomes (completion: success still shows the existing success toast, clears file browse context for the deleted branch, refreshes worktrees, and closes the dialog; failure still shows the existing error toast and leaves the worktree available after clearing the deleting indicator; verification: unit - dashboard tests cover successful pending-to-complete cleanup and failed pending-to-error cleanup without depending on real git operations).

- [x] Run dashboard quality gates (verification: manual - command output is captured in the implementation summary because these commands exercise local tooling rather than a persistent code artifact; completion: `npm run lint` and targeted Vitest tests for the modified dashboard components pass from `dashboard/`; behavior impact: no runtime behavior is added by this task).

## Future Work

If users later want progress that survives browser reloads, define a separate server-side asynchronous delete operation or active command broadcast contract.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-worktree-delete-deleting --archive-gate`

## Acceptance #1 Failure Follow-up
- [x] Add `deletingWorktreeBranch={deletingWorktreeBranch}` to the mobile `WorktreesPanel` render path and cover the mobile pending-delete state in a dashboard component test (verification: unit - `dashboard/src/App.worktree-delete.test.tsx` asserts all rendered WorktreesPanel instances receive `feature-a` while the delete promise is unresolved).

Evidence: `dashboard/src/App.tsx` passes `deletingWorktreeBranch={deletingWorktreeBranch}` to the mobile `WorktreesPanel`, and `dashboard/src/App.worktree-delete.test.tsx` covers the mobile path.

## Acceptance #2 Failure Follow-up
- [x] Normalize the Acceptance #1 follow-up evidence so OpenSpec task parsing does not treat metadata as an unchecked task (verification: manual - `openspec/changes/fix-worktree-delete-deleting/tasks.md` now uses prose evidence instead of a nested `- evidence:` task-like bullet, and `git diff --check -- openspec/changes/fix-worktree-delete-deleting/tasks.md` is the runnable command for whitespace sanity).

Previous failure evidence: the parser reported `tasks.md:24: Possible task without checkbox: - evidence: ...`. Implementation evidence from the prior follow-up remains `dashboard/src/App.tsx` passing `deletingWorktreeBranch={deletingWorktreeBranch}` to the mobile `WorktreesPanel`, with `dashboard/src/App.worktree-delete.test.tsx` covering the mobile path.

## Acceptance #3 Failure Follow-up
- [x] Move final OpenSpec validation evidence out of checkbox tasks and keep Acceptance follow-up entries free of parser-confusing nested task bullets (verification: manual - `openspec/changes/fix-worktree-delete-deleting/tasks.md` keeps metadata in prose and the non-checkbox Final Validation section; repository evidence is the edited `.md` source path plus `git diff --check -- openspec/changes/fix-worktree-delete-deleting/tasks.md`).

Acceptance #3 previous failure evidence: task parsing flagged a nested `- evidence:` item and final validation commands embedded inside checkbox tasks. The fix rewrites those entries so acceptance follow-up tasks describe repository edits, while final gate commands remain only in the non-checkbox `## Final Validation` section.

## Acceptance #4 Failure Follow-up
- [x] Add repository-verifiable evidence to Acceptance #2/#3 follow-up verification notes so completed manual verification tasks cite concrete artifacts (verification: manual - `openspec/changes/fix-worktree-delete-deleting/tasks.md` lines for Acceptance #2/#3 cite repository paths and the runnable command `git diff --check -- openspec/changes/fix-worktree-delete-deleting/tasks.md`).

Acceptance #4 previous failure evidence: `agent-exec run -- cflx openspec validate fix-worktree-delete-deleting --archive-gate` reported `tasks.md:28: Verification note should cite repository-verifiable evidence such as source paths, tests, or runnable commands` and `tasks.md:33: Verification note should cite repository-verifiable evidence such as source paths, tests, or runnable commands`. The fix updates those verification notes to cite `openspec/changes/fix-worktree-delete-deleting/tasks.md` and runnable `cflx openspec validate ...` commands.

## Acceptance #5 Failure Follow-up
- [x] Normalize Acceptance #2/#3/#4 follow-up verification text so checklist tasks cite repository-verifiable artifacts without self-referential final validation wording (verification: manual - edited `openspec/changes/fix-worktree-delete-deleting/tasks.md` lines 28, 33, and 38 so each completed task cites source paths and runnable commands such as `git diff --check -- openspec/changes/fix-worktree-delete-deleting/tasks.md`; final OpenSpec gate commands remain only in the non-checkbox `## Final Validation` prose).
