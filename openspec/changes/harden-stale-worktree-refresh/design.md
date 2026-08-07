# Design: Bounded Worktree Refresh

## Scope Boundary

The change modifies TUI worktree observation and VCS merge simulation diagnostics only. It does not alter orchestration lifecycle state or delete repository data.

## Eligibility

Refresh obtains the current active and rejected change IDs already derived from the repository change tree. A secondary worktree is automatically inspected only when its branch identity maps to one of those IDs. Other registered worktrees remain visible with an explicit not-inspected observation.

Rejected changes remain eligible because operators may need accurate merge/delete guidance while reviewing retained worktrees. Main worktree behavior remains unchanged.

## Observation Cache

The cache key contains:

- repository/base branch identity and HEAD;
- worktree branch identity and HEAD;
- merge base.

The value contains bounded ahead/conflict observation only. It is process-local, disposable, and never read by scheduler, resume, acceptance, archive, or merge execution. A key change creates a cache miss. No timer-based correctness assumption is required.

## Bounded Diagnostics

`git merge-tree` output is parsed into structured evidence. Logs and errors include total stdout/stderr byte counts and deterministic bounded samples. They never include the complete raw output. Known conflicts remain an ordinary observation rather than a command failure.

## Safety

Filtering prevents expensive commands but never removes rows or repository data. A not-inspected row is not equivalent to a clean row, so merge affordances remain fail-closed until a current eligible observation exists. Tests snapshot Git status and dirty file bytes before and after refresh.
