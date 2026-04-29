## MODIFIED Requirements

### Requirement: キュー変更デバウンスとスロット駆動の再分析

依存制約が解決した change は、依存解決後の実行開始時点で worktree を新規作成し、既存の worktree がある場合も作り直さなければならない（MUST）。この dependency-resolved recreation rule は通常 resume の例外として扱われ、依存に無関係な resumed worktree reuse を一般に禁止してはならない（MUST NOT）。

runtime は dependency blocked だった change が resolved になったことを記録し、次回 dispatch では generic resume ではなく forced fresh workspace creation を選択しなければならない（MUST）。既存 worktree/branch が存在する場合、それらは fresh dispatch 前に cleanup または equivalent invalidation され、stale worktree が再利用 source として残ってはならない（MUST NOT）。

#### Scenario: dependency-resolved change recreates worktree even when one already exists
- **GIVEN** change `beta` was previously blocked waiting for dependency `alpha`
- **AND** `beta` already has an older worktree created before `alpha` was merged
- **AND** dependency `alpha` is now resolved on the base branch
- **WHEN** the scheduler dispatches `beta` for its next execution start
- **THEN** the runtime does not reuse the older worktree
- **AND** the runtime creates a fresh worktree for `beta`
- **AND** the older worktree is cleaned up or otherwise invalidated before it can be reused

#### Scenario: normal resume still reuses worktree when dependency recreation rule does not apply
- **GIVEN** change `gamma` has an existing consistent worktree
- **AND** `gamma` was not previously blocked by unresolved dependencies
- **WHEN** the scheduler resumes `gamma`
- **THEN** the runtime may reuse the existing worktree
- **AND** dependency-resolved forced recreation is not triggered solely because resume occurred
