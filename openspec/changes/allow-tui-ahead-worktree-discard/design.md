## Context

Managed deletion is centralized in `WorktreeService`. It currently treats known commits ahead of base as an unconditional ineligibility, while known dirty state is a typed refusal that the local TUI can escalate into a second confirmation. Branch cleanup uses merged-only deletion so ordinary cleanup never makes commits unreachable.

The requested behavior introduces a second destructive authorization boundary: a local operator may intentionally discard an ahead branch, but remote callers and ordinary cleanup must remain unable to do so.

## Decisions

### Use a typed ahead refusal as the escalation boundary

`classify_delete_eligibility` will return an ahead-specific typed refusal carrying the freshly observed path, Git worktree identity, branch, HEAD, and dirty classification when commits are known ahead and ahead-discard permission is absent. The TUI opens the destructive modal only from this service result, not from the potentially stale worktree list projection.

If the worktree is both dirty and ahead, ahead classification remains first. Its target includes the known dirty fact so one modal can explicitly disclose both losses. Uppercase `X` then grants both permissions in the submitted intent. This is not implicit permission composition: the combined loss is named before the one keypress authorizes it.

### Keep three permissions independent

Deletion policy carries separate booleans for:

- skipping teardown;
- discarding known uncommitted changes;
- discarding known commits ahead of base and deleting their branch.

Ordinary local deletion sets only the teardown choice. Dirty-only confirmation sets only known-dirty discard. Ahead confirmation sets ahead discard and, only when the typed target says dirty, known-dirty discard. Remote callers set none.

### Revalidate before each irreversible boundary

The service re-observes after teardown and before worktree removal. It requires the expected worktree identity, path, branch, and HEAD to remain stable; dirty, ahead, and base-merge state must remain known and authorized. `confirm_branch_ref` must still prove the branch ref equals the observed HEAD before worktree removal.

After worktree removal, explicit ahead-branch deletion reads the branch ref again. It force-deletes only the branch ref that still equals the confirmed HEAD. A moved or unreadable ref is retained and reported; the already removed worktree is not reconstructed.

### Separate ordinary and destructive branch cleanup

Ordinary deletion continues to call merged-only branch cleanup. Explicit ahead discard uses a distinct backend operation that can delete an unmerged local branch. The destructive operation is not selected from `has_commits_ahead` alone; it requires the explicit local permission and confirmed target evidence.

### Keep remote deletion fail-closed

No remote DTO or API parameter changes. Remote projection continues to evaluate deletion with `DeleteOptions::fail_closed()`, so ahead worktrees remain undeletable with a server-provided reason.

## Alternatives Considered

### Delete the worktree but retain the ahead branch

Rejected by operator choice. It preserves commits but does not satisfy the requested cleanup of both resources.

### Add a general force flag

Rejected because it would collapse independent safety decisions and could accidentally waive unknown observations, main status, merge state, or identity drift.

### Force-delete the branch before removing the worktree

Rejected because Git cannot safely delete a checked-out branch and because branch loss before teardown/worktree removal would create a worse partial-failure state.

## Failure Semantics

- Refusal or teardown failure before worktree removal preserves both resources.
- Worktree removal failure preserves the branch.
- Branch ref drift or branch deletion failure after worktree removal retains the branch and returns/logs partial success.
- Unknown safety facts always fail closed.

## Verification Strategy

Unit tests at the service boundary cover policy combinations, teardown ordering, second observation, identity/ref drift, backend call ordering, and partial success. TUI state/render/key tests cover disclosure, uppercase-`X` exclusivity, cancellation, fresh-target revalidation, and deletion progress. Remote projection/API tests prove no destructive option is exposed and ahead deletion remains blocked.
