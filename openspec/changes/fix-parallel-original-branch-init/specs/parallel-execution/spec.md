## MODIFIED Requirements

### Requirement: Parallel Execution Event Reporting

order-based再分析ループでもarchive完了後のmerge結果に応じてイベントを送信し、merge成功時にはcleanupイベントを送信しなければならない（SHALL）。

MergeDeferred が発生した場合は `MergeDeferred` イベントを送信し、待ち状態の表示は TUI 仕様に従って `MergeWait` または `ResolveWait` を判定しなければならない（SHALL）。

さらに、`MergeDeferred` のうち先行 merge / resolve の完了で再評価可能な change は、自動再評価対象として保持されなければならない（MUST）。
先行 merge または resolve が完了したとき、システムは自動再評価対象の change を再評価し、競合が残る場合は `ResolveWait` または `Resolving` に進め、merge 再試行可能な場合は `MergeWait` に留めてはならない（MUST）。
手動介入が必要な change のみが `MergeWait` に留まらなければならない（MUST）。

Git backend では archive-complete 後の merge/dependency 判定に先立って base branch (`original_branch`) を初期化しなければならない（MUST）。初期化可能な場合、システムは self-heal して merge handling を継続し、`Original branch not initialized` を理由に archived change を `MergeWait` に留めてはならない（MUST）。recover 不能な detached HEAD 等のみが実行エラーとして報告されてよい（MAY）。

#### Scenario: archived merge self-heals when base branch was not yet initialized
- **GIVEN** a parallel Git worktree has already completed archive
- **AND** the archived change is being handed off into merge handling
- **AND** the workspace manager has not yet cached `original_branch`
- **WHEN** merge handling starts
- **THEN** the system initializes the base branch from the repository state before merge evaluation
- **AND** merge handling continues without surfacing `Original branch not initialized`
- **AND** the change does not remain in `MergeWait` solely due to the missing initialization

#### Scenario: unrecoverable base branch discovery fails as execution error
- **GIVEN** a parallel Git worktree has already completed archive
- **AND** merge handling cannot determine a base branch because the repository is in detached HEAD state
- **WHEN** merge handling starts
- **THEN** the system reports an execution error rather than classifying the change as manual-intervention `MergeWait`
- **AND** the failure is distinguishable from deferred merge conflicts or base-dirty waits
