## MODIFIED Requirements
### Requirement: Workspace Cleanup Guard
order-based再分析ループでは、MergeWaitのchangeに対応するworktreeをcleanupから除外し、`WorkspaceCleanupGuard`のDropで削除されないようにしなければならない（MUST）。

#### Scenario: MergeWaitのworktreeはcleanupから除外される
- **GIVEN** order-based再分析ループで変更Aが `MergeDeferred` になっている
- **AND** 変更Aのworktreeが `WorkspaceCleanupGuard` にトラッキングされている
- **WHEN** 正常系のcleanupまたはガードのDropが実行される
- **THEN** 変更Aのworktreeは削除されない
