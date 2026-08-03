## ADDED Requirements

### Requirement: Explicit dirty discard is a local deletion permission

Known dirty SHALL mean tracked/index changes or non-ignored untracked entries reported using an explicit untracked-file status mode. Ignored-only content SHALL NOT by itself classify the worktree as `Dirty` and MAY still be removed with the directory.

Dirty-discard permission MAY waive only known `Dirty`. It MUST NOT waive unknown dirty state, unknown commits-ahead state, unknown base-merge state, main-worktree status, active/deleting state at the TUI boundary, expected Git worktree identity/ref mismatch, or known commits ahead. The permission MUST default to disabled and only the local TUI destructive-confirmation path MAY enable it. Remote deletion MUST remain fail-closed and MUST NOT expose it.

Safety-critical target facts MUST be re-observed after teardown and immediately before Git removal. Branch cleanup MUST retain the branch and warn when its ref moved or safe reachability cannot be reconfirmed.

#### Scenario: Ordinary deletion refuses known dirty state

**Given**: A managed worktree has tracked/index changes or reported non-ignored untracked entries
**And**: Dirty-discard permission is disabled
**When**: Managed deletion is evaluated
**Then**: Deletion returns a typed dirty refusal before teardown or removal

#### Scenario: Explicit local dirty discard proceeds

**Given**: A non-main managed worktree is known dirty and has no commits ahead
**And**: Its expected Git identity, branch, HEAD/ref, merge state, and required observations are valid
**When**: The local TUI supplies explicit dirty-discard permission after destructive confirmation
**Then**: Deletion may proceed after final re-observation
**And**: Teardown runs unless independently skipped
**And**: A structured warning records the explicit discard before forced removal

#### Scenario: Unknown safety observation refuses deletion

**Given**: Dirty, base, ahead, merge, identity, or ref safety state cannot be determined
**When**: Ordinary or explicit dirty-discard deletion is evaluated
**Then**: Deletion is refused
**And**: The worktree and branch are retained

#### Scenario: Teardown-induced drift refuses removal

**Given**: Initial deletion checks pass
**And**: Teardown changes target identity, branch/ref, HEAD, merge state, or commits-ahead safety
**When**: The system re-observes immediately before Git removal
**Then**: Forced worktree removal is not invoked
**And**: The target is retained with diagnostics

#### Scenario: Skip teardown remains independent

**Given**: A worktree is known dirty
**And**: Skip-teardown is enabled but dirty-discard permission is disabled
**When**: Deletion is evaluated
**Then**: Deletion returns the dirty refusal
**And**: Neither teardown nor Git removal runs

#### Scenario: Branch ref drift preserves branch

**Given**: Worktree removal was authorized from a validated branch OID
**And**: The branch ref moves or reachability cannot be reconfirmed before cleanup
**When**: Best-effort branch cleanup runs
**Then**: The branch is retained
**And**: A warning records why cleanup was skipped

#### Scenario: Ignored-only content is not a dirty escalation

**Given**: A worktree contains only ignored/generated files beyond committed content
**When**: Explicit status observation runs without ignored-file enumeration
**Then**: The worktree may classify clean
**And**: The ordinary deletion warning states directory contents may be permanently removed

#### Scenario: Remote callers cannot request dirty discard

**Given**: A remote client addresses a dirty managed worktree
**When**: It requests ordinary deletion or submits dirty-discard, force, teardown-skip, path, or branch parameters
**Then**: Ordinary deletion is refused as dirty or the unsafe request shape is rejected
**And**: Removal is not delegated
