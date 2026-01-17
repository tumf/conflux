# Fix Worktree Merge Conditions

## Why

The TUI Worktree View merge functionality has the following UX issues and bugs that confuse users:

1. The M key is displayed even for worktrees that don't need merging (at the same commit position as the base branch), but pressing it does nothing
2. When pressing M key fails, no error message is shown, making it impossible to understand the cause
3. UI display conditions and logic execution conditions are inconsistent
4. **Merge execution location is incorrect** - merge is executed on the worktree side, but should be executed on the base (main worktree) side
5. **TUI may crash** - there are reports of TUI silently exiting when M key is pressed, requiring debug logging and stability improvements

This fix enables users to:
- Display M key only when merge is actually possible, preventing wasted operations
- See clear error messages on failure to understand what the problem is
- Correctly merge worktree branches to the base side
- Perform stable merge operations without crashes
- Use a more reliable worktree merge feature

## What Changes

The following issues occur in TUI Worktree View:

1. **M key is always displayed** - M key is shown even when merge is unnecessary (worktree not ahead of base branch)
2. **Pressing M key doesn't execute command** - internal condition check returns `None` but no error message is displayed
3. **Lack of user feedback** - users can't understand why merge is not possible
4. **Merge execution location is wrong** - currently executes `merge_branch(&worktree_path, ...)` on worktree side, but should execute `merge_branch(&repo_root, ...)` on base side. This causes "Working directory is not clean" errors due to dirty state on worktree side
5. **TUI crashes** - TUI sometimes exits silently when M key is pressed. Debug logging is insufficient for troubleshooting

### Proposed Changes

#### 1. Strict M Key Display Conditions

Display M key only when ALL of the following conditions are met:

- Not main worktree
- Not detached HEAD
- No merge conflicts
- Has branch name
- **Has commits ahead of base branch** (NEW)

#### 2. Add Error Messages

Display appropriate warning messages when `request_merge_worktree_branch()` condition checks fail:

- When view_mode is different: "Switch to Worktrees view to merge"
- When worktrees is empty: "No worktrees loaded"
- When cursor is out of range: "Cursor out of range: {cursor} >= {len}"
- Keep existing messages (main/detached/conflict/no branch)

#### 3. Extend WorktreeInfo

Add `has_commits_ahead: bool` field and check for differences when loading worktrees.

#### 4. Fix Merge Execution Location

Current implementation (incorrect):
```rust
// runner.rs:1110
merge_branch(&worktree_path, &merge_branch)  // Execute merge on worktree side
```

Fixed:
```rust
merge_branch(&merge_repo_root, &merge_branch)  // Execute merge on base (main worktree) side
```

This ensures:
- Working directory clean check is performed on base side
- Merge commit is created on base side
- Uncommitted changes on worktree side don't interfere

#### 5. Strengthen Debug Logging and Error Handling

To investigate crash issues and improve stability:

- Add debug logging to `request_merge_worktree_branch()`
- Add debug logging to M key handling
- Strengthen error handling during command send/receive
- Display warnings instead of exiting TUI on unexpected errors

## Impact Scope

- `src/tui/types.rs` - WorktreeInfo structure
- `src/tui/runner.rs` - worktree loading process, **fix merge execution location**, add debug logging
- `src/tui/render.rs` - M key display conditions
- `src/tui/state/mod.rs` - merge request processing, add debug logging
- `src/vcs/git/commands.rs` - add difference check function (if needed)

## Expected Results

- M key is displayed only when merge is actually possible
- Clear error messages are shown when pressing M key fails
- Users understand why merge is not possible
- **Worktree branches are correctly merged to base (main worktree) side**
- **Uncommitted changes on worktree side don't block merge**
- **TUI doesn't crash, and debug logs help identify issues when problems occur**
