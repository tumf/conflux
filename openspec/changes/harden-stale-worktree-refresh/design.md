# Design: Bounded Worktree Refresh

## Scope Boundary

The change modifies the shared TUI and Web/UDS worktree observation paths plus VCS merge simulation diagnostics. It does not alter orchestration lifecycle state or delete repository data.

## Eligibility

Both periodic paths, TUI `load_worktrees_with_conflict_check` and Web/UDS `refresh_from_disk` through `observe_worktrees`, obtain the current active and rejected change IDs already derived from the repository change tree. A secondary worktree is automatically inspected only when `extract_change_id_from_worktree_name` maps its branch identity to one of those IDs. Other registered worktrees remain visible with an explicit not-inspected observation.

Filtering applies only to periodic automatic refresh. An operator-initiated merge or deletion performs a fresh targeted observation for that worktree before eligibility is decided, preserving stale-worktree cleanup and `ws-session-*` merge behavior. Rejected changes remain periodically eligible because operators may need accurate guidance while reviewing retained worktrees. Main worktree behavior remains unchanged.

## Observation Cache

The cache key contains:

- repository/base branch identity and HEAD;
- worktree branch identity and HEAD;
- merge base.

The value contains bounded ahead/conflict observation only. One shared observation/cache layer serves both periodic TUI and Web/UDS refresh paths so the same tuple is simulated at most once per process. It is process-local and disposable. Current-keyed observations may drive display and merge affordances, but merge and delete execution revalidate repository state and scheduler, resume, acceptance, archive, and next-action logic never read the cache. A key change creates a cache miss. No timer-based correctness assumption is required.

## Bounded Diagnostics

`git merge-tree` output is parsed into structured evidence. Logs and errors include total stdout/stderr byte counts and deterministic bounded samples capped at 20 conflict paths and 4096 bytes for each stdout/stderr prefix. They never include the complete raw output. Known conflicts remain an ordinary observation rather than a command failure.

## Safety

Filtering prevents expensive commands but never removes rows or repository data. A not-inspected row is not equivalent to a clean row, so merge affordances remain fail-closed until a current eligible observation exists. Tests snapshot Git status and dirty file bytes before and after refresh.
