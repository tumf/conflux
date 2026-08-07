## MODIFIED Requirements

### Requirement: Auto-Refresh Worktree List

Worktreeリスト SHALL be automatically refreshed without modifying tracked files in worktrees.

All Git-registered worktrees SHALL remain visible. Automatic ahead/conflict inspection SHALL run only for the main worktree and secondary worktrees whose branch identity maps to a currently active or rejected OpenSpec change. A worktree that is not eligible for automatic inspection SHALL expose an explicit not-inspected state and MUST NOT be treated as conflict-free or mergeable.

衝突チェックは作業ツリーに影響を与えないGit手法で実行し、worktree上の作業状態を変更してはならない。

衝突チェックで `git merge-tree` を利用する場合、正しい引数形式で実行し、競合時はエラー扱いではなく競合ありとして判定しなければならない（MUST）。

Inspection observations MAY be reused only while branch identity, base HEAD, worktree HEAD, and merge base are unchanged. Reuse state MUST remain process-local observability state and MUST NOT become an input to scheduler dispatch, resume routing, acceptance, archive, merge execution, or next-action selection.

<!-- Expected canonical result after archive: Worktrees refresh retains every registered worktree while limiting automatic merge simulation to current change worktrees and safely reusing unchanged Git-derived observations. -->

#### Scenario: Non-active worktree remains visible without merge simulation

- **GIVEN** a registered secondary worktree branch does not map to a current active or rejected change
- **WHEN** the periodic Worktrees refresh runs
- **THEN** the worktree remains present in the listing
- **AND** no ahead or conflict simulation command is spawned for that worktree
- **AND** its observation is marked not inspected rather than conflict-free

#### Scenario: Unchanged active worktree reuses observation

- **GIVEN** an active change worktree has a completed ahead/conflict observation
- **AND** its branch identity, base HEAD, worktree HEAD, and merge base remain unchanged
- **WHEN** another periodic refresh runs
- **THEN** the previous observation is reused
- **AND** no duplicate merge simulation is executed

#### Scenario: Revision change invalidates observation

- **GIVEN** an active change worktree has a cached observation
- **WHEN** its branch identity, base HEAD, worktree HEAD, or merge base changes
- **THEN** the cached observation is not reused
- **AND** a fresh non-mutating inspection is executed

#### Scenario: Refresh preserves dirty worktree state

- **GIVEN** an eligible or ineligible worktree contains staged, unstaged, or untracked work
- **WHEN** periodic refresh runs
- **THEN** its index and file bytes remain unchanged
- **AND** no cleanup, stash, commit, reset, rebase, or merge mutation is performed
