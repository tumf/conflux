## ADDED Requirements

### Requirement: Dashboard worktree deletion shows row-level pending state

The server-mode dashboard SHALL show a per-worktree pending deletion state for the worktree branch whose delete request is currently in flight, without requiring users to infer progress from the global loading state or confirmation dialog alone.

#### Scenario: Target worktree row shows deleting state while delete is pending

**Given**: the dashboard Worktrees panel is rendering worktrees `feature-a` and `feature-b`
**When**: the user confirms deletion of worktree branch `feature-a` and the delete API request has not completed
**Then**: the `feature-a` worktree row displays a visible deleting indicator with a spinner or equivalent progress affordance
**And**: the `feature-b` worktree row does not display the deleting indicator

#### Scenario: Deleting worktree row suppresses conflicting interactions

**Given**: worktree branch `feature-a` is currently being deleted from the dashboard
**When**: the dashboard renders the `feature-a` worktree row
**Then**: merge and delete controls for that row are disabled or unavailable
**And**: clicking the row does not change the active file-browse worktree selection

#### Scenario: Deleting state clears after delete success

**Given**: worktree branch `feature-a` is currently displayed as deleting
**When**: the delete API request succeeds
**Then**: the dashboard clears the deleting indicator for `feature-a`
**And**: refreshes the worktree list
**And**: clears any file-browse context that referenced `feature-a`

#### Scenario: Deleting state clears after delete failure

**Given**: worktree branch `feature-a` is currently displayed as deleting
**When**: the delete API request fails
**Then**: the dashboard clears the deleting indicator for `feature-a`
**And**: leaves the worktree row available in the current list
**And**: surfaces the existing delete failure error to the user
