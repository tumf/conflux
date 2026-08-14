## ADDED Requirements

### Requirement: Apply post-quiescence index-lock residue converges before repository finalization

After one owned Apply command reaches Unix-confirmed process-group quiescence, Conflux MUST resolve the current managed worktree's `index.lock` residue before starting any repository observation that may refresh or mutate the index, WIP snapshot, final Apply commit, cleanup review, rejecting handoff, or Acceptance handoff. Conflux MAY unlink an `index.lock` only when process-lifetime evidence for that same Apply dispatch proves the managed lock candidate was absent immediately before spawn, and two post-quiescence observations separated by a fixed 500 millisecond dwell prove that the pathname remains a regular, non-symlink, zero-byte file with the same device, inode, and modification time. The second observation MUST use a no-follow file descriptor and descriptor metadata. Conflux MUST treat disappearance or unlink `ENOENT` as natural convergence and MUST fail closed without repository finalization or handoff for every unsupported, pre-existing, ambiguous, changed, non-zero, or failed observation.

Reclamation authority MUST be limited to the post-quiescence Apply boundary. Git retry policies MUST NOT unlink a lock. Conflux MUST NOT use `lsof`, file age, wall-clock timestamp windows, PID ownership, or process-group attribution as deletion authority. The pre-dispatch observation MUST remain ephemeral, MUST NOT survive process restart, and MUST NOT influence restart routing.

#### Scenario: same-dispatch orphaned zero-byte lock is reclaimed

- **GIVEN** the managed worktree `index.lock` is absent immediately before an Apply command is spawned
- **AND** the command creates a zero-byte regular `index.lock` and ends with Unix-confirmed process-group quiescence
- **AND** both post-quiescence observations identify the same unchanged device, inode, zero size, and modification time
- **WHEN** Conflux reaches the repository-finalization barrier
- **THEN** Conflux unlinks that lock or observes that it naturally disappeared
- **AND** normal WIP, final-commit, and handoff gates may continue

#### Scenario: pre-existing lock is never reclaimed by the dispatch

- **GIVEN** an `index.lock` candidate exists before the Apply command is spawned
- **WHEN** that path remains after process-group cleanup
- **THEN** Conflux does not unlink it even when it is regular and zero-byte
- **AND** repository finalization and handoff fail closed with diagnostics identifying pre-existence

#### Scenario: unstable or unsafe file evidence refuses reclamation

- **GIVEN** an Apply command reaches confirmed process-group quiescence
- **AND** the remaining path is a symlink, non-regular file, non-zero file, unreadable file, or changes device, inode, size, or modification time during the dwell
- **WHEN** Conflux evaluates post-quiescence residue
- **THEN** Conflux does not unlink the path
- **AND** no WIP snapshot, final Apply commit, cleanup review, rejecting handoff, or Acceptance starts
- **AND** diagnostics identify the failed evidence condition

#### Scenario: unconfirmed or unsupported cleanup has no deletion authority

- **GIVEN** Apply cleanup reports `NotApplicable`, `MembersRemain`, or `Unverifiable`
- **WHEN** Conflux reaches the repository-finalization barrier
- **THEN** Conflux does not inspect the lock as a reclaimable same-dispatch orphan
- **AND** it does not unlink the lock
- **AND** existing process-group cleanup diagnostics block finalization and handoff

#### Scenario: interrupted progress preservation uses the same convergence boundary

- **GIVEN** cancellation, external shutdown, or the absolute runtime limit interrupts an Apply command
- **WHEN** owned cleanup confirms Unix process-group quiescence
- **THEN** Conflux resolves same-dispatch `index.lock` residue through the same evidence and refusal rules before inspecting dirtiness or creating a progress snapshot
- **AND** an unsafe or unverifiable residue leaves workspace and index contents untouched and returns a terminal diagnostic

#### Scenario: restart cannot reclaim from expired provenance

- **GIVEN** an Apply process exits with an `index.lock` remaining
- **WHEN** a new Conflux process resumes the workspace without the original pre-dispatch observation
- **THEN** it does not infer same-dispatch reclamation authority from file age, logs, or repository state
- **AND** it leaves the lock untouched for explicit recovery

<!-- Expected canonical result after archive: `process-execution` will require same-dispatch, two-point, fail-closed convergence of orphaned zero-byte managed-worktree index locks after confirmed Apply quiescence and before repository finalization. -->
