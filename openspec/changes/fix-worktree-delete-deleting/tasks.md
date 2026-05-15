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
- [x] dashboard/src/App.tsx:761-773 のモバイル用 WorktreesPanel 呼び出しに deletingWorktreeBranch={deletingWorktreeBranch} が渡されていません。提案の明示条件（proposal.md:52）とタスク（tasks.md:5）は desktop/mobile 両方の render path に delete progress branch を通すことを要求していますが、現状は desktop 側 App.tsx:633-645 のみ渡しているため、モバイルの Worktrees タブでは削除リクエスト pending 中でも該当行に deleting indicator が表示されません。修正として mobile 側 WorktreesPanel に同じ deletingWorktreeBranch prop を追加し、モバイル経路を検証するテストを追加してください。
  - evidence: `dashboard/src/App.tsx` のモバイル `WorktreesPanel` に `deletingWorktreeBranch={deletingWorktreeBranch}` を追加し、`dashboard/src/App.worktree-delete.test.tsx` にモバイル経路で pending 中の全 WorktreesPanel が対象 branch を受け取るテストを追加。
