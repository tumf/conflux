## ADDED Requirements

### Requirement: Final Apply commit retries narrowly classified index-lock contention

After Apply process-group quiescence is confirmed, Conflux MAY retry final Apply lock-acquisition contention under an immutable finalization plan. The plan MUST freeze baseline HEAD, add-and-commit or amend mode, exact subject, expected tree, and expected lineage before mutation. Each retry MUST recognize exact success or fail closed unless HEAD and the complete isolated-index workspace tree still match the plan. Conflux MUST NOT switch mode, absorb external drift, rerun after hooks executed, delete the lock, or bypass verification hooks.

#### Scenario: finalization requires confirmed quiescence

- **GIVEN** Apply completion was observed
- **AND** process-group cleanup is unconfirmed
- **WHEN** final Apply lock retry would otherwise start
- **THEN** Conflux performs no retry or final commit attempt
- **AND** Apply fails through the cleanup barrier

#### Scenario: transient add-and-commit lock clears without drift

- **GIVEN** confirmed process-group quiescence
- **AND** an immutable dirty-worktree plan freezes baseline HEAD, `AddAndCommit`, and the isolated-index expected tree
- **AND** eligible managed-worktree lock contention occurs before hooks execute
- **WHEN** HEAD and the complete workspace tree remain unchanged and the lock clears within three attempts
- **THEN** Conflux stages only the frozen expected tree and creates one hook-enabled commit
- **AND** that commit has baseline HEAD as its sole parent, exact subject, and expected tree

#### Scenario: transient amend lock clears without drift

- **GIVEN** confirmed process-group quiescence
- **AND** an immutable clean-worktree plan freezes baseline HEAD, `Amend`, baseline parents, and expected tree
- **AND** eligible managed-worktree lock contention occurs before hooks execute
- **WHEN** repository state remains unchanged and the lock clears within three attempts
- **THEN** Conflux creates one replacement commit with the baseline ordered parent set, exact subject, and expected tree
- **AND** it does not amend any later external commit

#### Scenario: repository drift fails closed

- **GIVEN** a finalization plan exists
- **AND** a retry boundary observes external HEAD advance, mode change, index drift, or staged, unstaged, deleted, or untracked content differing from the expected tree
- **WHEN** Conflux performs retry preflight
- **THEN** it returns a terminal concurrent-mutation diagnostic before another stage or commit
- **AND** it does not reset, absorb, amend, or commit the external change

#### Scenario: lock-failed attempts do not rerun hooks

- **GIVEN** a counting repository hook and eligible top-level Git lock-acquisition failures
- **WHEN** Conflux retries and eventually succeeds
- **THEN** lock-failed attempts have executed the hook zero times
- **AND** the successful verified commit executes it exactly once
- **AND** a hook rejection uses existing Apply repair rather than lock retry

#### Scenario: persistent or unrelated failures remain terminal

- **GIVEN** contention persists for three attempts, hook execution cannot be excluded, or failure concerns another lock, malformed stderr, permission, configuration, conflict, backend, or command
- **WHEN** Conflux classifies the failure
- **THEN** it returns structured terminal diagnostics and preserves workspace state
- **AND** it does not delete the lock or consume another retry
