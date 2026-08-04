## MODIFIED Requirements

### Requirement: Explicit dirty discard is a local deletion permission

Known dirty SHALL mean tracked/index changes or non-ignored untracked entries reported using an explicit untracked-file status mode. Ignored-only content SHALL NOT by itself classify the worktree as `Dirty` and MAY still be removed with the directory.

Dirty-discard permission and commits-ahead-discard permission SHALL be independent local deletion permissions. Dirty-discard permission MAY waive only known `Dirty`. Commits-ahead-discard permission MAY waive only known commits ahead of base and MAY authorize deletion of the confirmed local branch after worktree removal. Neither permission SHALL waive unknown dirty state, unknown commits-ahead state, unknown base-merge state, main-worktree status, active/deleting state at the TUI boundary, expected Git worktree identity/ref mismatch, or a permission it does not explicitly grant. Both permissions MUST default to disabled, and only the local TUI destructive-confirmation paths MAY enable them. Remote deletion MUST remain fail-closed and MUST NOT expose either permission.

Safety-critical target facts MUST be re-observed after teardown and immediately before Git removal. Ordinary branch cleanup MUST retain the branch and warn when its ref moved or safe reachability cannot be reconfirmed. Explicit ahead-branch cleanup MUST read the ref again after worktree removal and delete the unmerged branch only when it still points to the confirmed HEAD; otherwise it MUST retain the branch and report partial success.

#### Scenario: Ordinary deletion refuses known dirty state

**Given**: A managed worktree has tracked/index changes or reported non-ignored untracked entries
**And**: The worktree has no commits ahead
**And**: Dirty-discard permission is disabled
**When**: Managed deletion is evaluated
**Then**: Deletion returns a typed dirty refusal before teardown or removal

#### Scenario: Ordinary deletion escalates a clean ahead worktree

**Given**: A non-main managed worktree is clean and has commits ahead of base
**And**: Commits-ahead-discard permission is disabled
**When**: Local managed deletion is evaluated
**Then**: Deletion returns a typed ahead refusal containing fresh path, identity, branch, HEAD, and dirty evidence
**And**: Neither teardown, worktree removal, nor branch deletion runs

#### Scenario: Explicit local dirty discard proceeds

**Given**: A non-main managed worktree is known dirty and has no commits ahead
**And**: Its expected Git identity, branch, HEAD/ref, merge state, and required observations are valid
**When**: The local TUI supplies explicit dirty-discard permission after destructive confirmation
**Then**: Deletion may proceed after final re-observation
**And**: Teardown runs unless independently skipped
**And**: A structured warning records the explicit discard before forced removal

#### Scenario: Explicit local ahead discard deletes worktree and branch

**Given**: A non-main managed worktree has commits ahead of base
**And**: The local TUI displays its path, branch, HEAD, teardown choice, and permanent loss of unmerged commits without stash, backup, or merge
**And**: The operator confirms with uppercase `X`
**When**: Identity, branch, HEAD/ref, dirty, ahead, and base-merge facts remain known and authorized through final re-observation
**Then**: The worktree is removed
**And**: The local branch is force-deleted only if its ref still equals the confirmed HEAD
**And**: A structured warning records the explicit ahead discard

#### Scenario: Dirty and ahead discard requires combined disclosure

**Given**: A managed worktree is both known dirty and known ahead of base
**When**: Ordinary local deletion returns a typed ahead refusal
**Then**: The destructive confirmation explicitly states that uncommitted changes and unmerged commits will both be lost
**And**: Uppercase `X` grants both permissions for that confirmed target
**And**: Dirty-only or ahead-only permission cannot delete the target

#### Scenario: Destructive confirmation keys are fail-safe

**Given**: The local TUI displays the ahead-discard confirmation
**When**: The operator presses `Y`, `S`, lowercase `x`, or an unrelated key
**Then**: No deletion command is emitted
**When**: The operator presses `N` or Escape
**Then**: The confirmation closes and both worktree and branch are retained

#### Scenario: Unknown safety observation refuses deletion

**Given**: Dirty, base, ahead, merge, identity, or ref safety state cannot be determined
**When**: Ordinary or explicitly authorized deletion is evaluated
**Then**: Deletion is refused
**And**: The worktree and branch are retained

#### Scenario: Teardown-induced drift refuses removal

**Given**: Initial deletion checks and any required destructive confirmation pass
**And**: Teardown changes target identity, branch/ref, HEAD, dirty, merge, or commits-ahead safety
**When**: The system re-observes immediately before Git removal
**Then**: Forced worktree removal is not invoked
**And**: The target is retained with diagnostics

#### Scenario: Skip teardown remains independent

**Given**: A worktree is known dirty or known ahead
**And**: Skip-teardown is enabled but the required discard permission is disabled
**When**: Deletion is evaluated
**Then**: Deletion returns the applicable typed refusal
**And**: Neither teardown nor Git removal runs

#### Scenario: Branch ref drift after removal preserves branch

**Given**: Worktree removal was explicitly authorized from a validated branch OID
**And**: The worktree was removed
**And**: The branch ref then moves or cannot be reconfirmed
**When**: Explicit ahead-branch cleanup runs
**Then**: The branch is retained
**And**: The outcome reports partial success and why branch deletion was skipped

#### Scenario: Ignored-only content is not a dirty escalation

**Given**: A worktree contains only ignored/generated files beyond committed content
**When**: Explicit status observation runs without ignored-file enumeration
**Then**: The worktree may classify clean
**And**: The ordinary deletion warning states directory contents may be permanently removed

#### Scenario: Remote callers cannot request destructive discard

**Given**: A remote client addresses a dirty or ahead managed worktree
**When**: It requests ordinary deletion or submits dirty-discard, commits-ahead-discard, force, teardown-skip, path, or branch parameters
**Then**: Ordinary deletion is refused by the applicable safety guard or the unsafe request shape is rejected
**And**: The worktree and branch are retained
