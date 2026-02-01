# parallel-analysis Specification

## Purpose
TBD - created by archiving change pass-approved-changes-to-analyzer. Update Purpose after archive.
## Requirements
### Requirement: Parallel dependency analysis prompt

The dependency analysis prompt SHALL clearly mark selected changes (`is_approved = true`) and include the proposal file path for each change.

The prompt SHALL include:
- Mark selected changes with `[x]`
- Mark unselected changes with `[ ]` (for future extensibility)
- The full file path for each change (`openspec/changes/{change_id}/proposal.md`)
- An explicit instruction to analyze only selected changes
- Response instructions to return both `order` (recommended execution order after honoring dependencies) and `dependencies`
- A statement that `dependencies` apply only when one change explicitly depends on the artifacts, specifications, or APIs of another and cannot be established without them
- A statement that `order` is a recommended sequence based on priority or efficiency and is independent of dependencies

プロンプト構築と出力解析は別関数に分割してもよい（MAY）。ただし、プロンプト内容と選別ルールは既存と同一でなければならない（MUST）。

#### Scenario: Return dependencies only for mandatory conditions
- **GIVEN** multiple changes are included in the dependency analysis
- **AND** one change requires another's artifacts, specifications, or APIs as a mandatory condition
- **WHEN** dependency analysis is executed
- **THEN** `dependencies` includes only the mandatory relationships
- **AND** relationships based only on priority or ordering preference are excluded from `dependencies`
