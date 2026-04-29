## ADDED Requirements

### Requirement: Rejecting recovery must update canonical tasks location for archived workspaces

Rejection review が `RESUME` または `BLOCK` を返した後、runtime は recovery task を current workspace 内で canonical な `tasks.md` に追記しなければならない（MUST）。

active change directory (`openspec/changes/<change_id>/tasks.md`) が存在しない場合、runtime は archived workspace entry (`openspec/changes/archive/<date>-<change_id>/tasks.md` または同等 path) を探索し、存在する archive tasks file を更新しなければならない（MUST）。

active path が無いことだけを理由に change を terminal `Error` にしてはならない（MUST NOT）。active path と archive path の両方が存在しない場合のみ、探索した path 一覧を含む実行エラーを返してよい（MAY）。

#### Scenario: archived workspace resumes from rejecting review without active tasks path

- **GIVEN** a change has already been archived in its worktree and `openspec/changes/<change_id>/tasks.md` no longer exists
- **AND** `openspec/changes/archive/<date>-<change_id>/tasks.md` exists
- **WHEN** rejecting review returns `REJECTION_REVIEW: RESUME`
- **THEN** the runtime appends the recovery task to the archived tasks file
- **AND** the change transitions back to applying rather than failing with file-not-found

#### Scenario: archived workspace blocks from rejecting review without active tasks path

- **GIVEN** a change has already been archived in its worktree and only the archived tasks file exists
- **WHEN** rejecting review returns `REJECTION_REVIEW: BLOCK`
- **THEN** the runtime appends the recovery task to the archived tasks file
- **AND** the change transitions to blocked rather than terminal error

#### Scenario: rejecting recovery reports explored paths when neither active nor archived tasks file exists

- **GIVEN** neither the active change tasks path nor any archived tasks path exists for a change
- **WHEN** rejecting recovery attempts to append a recovery task
- **THEN** the runtime returns an execution error
- **AND** the error message includes the explored active path and archive path candidates
