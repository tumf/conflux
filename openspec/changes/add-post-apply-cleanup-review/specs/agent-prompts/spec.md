## ADDED Requirements

### Requirement: cflx-workflow MUST support cleanup-review operation prompts

Conflux の orchestrator は、managed worktree apply の post-apply handoff cleanup のために `cflx-workflow` skill の cleanup-review operation を呼び出せなければならない（MUST）。cleanup-review prompt は blind staging を禁止し、worktree を clean handoff-ready 状態にする責務を与え、成功時のみ machine-readable marker を 1 回だけ返すよう指示しなければならない（MUST）。

#### Scenario: Cleanup-review prompt loads cflx-workflow with dedicated operation context

- **GIVEN** orchestrator が task-complete だが dirty な managed worktree を検出した
- **WHEN** cleanup-review prompt を構築する
- **THEN** prompt は `load skills: cflx-workflow` を含む
- **AND** prompt は cleanup-review 専用 operation を識別できる prelude を含む
- **AND** prompt は change_id と relevant paths を含む

#### Scenario: Cleanup-review prompt forbids blind staging

- **GIVEN** cleanup-review prompt が生成される
- **WHEN** agent が handoff cleanup を実行する
- **THEN** prompt は blind `git add -A` や dirty file 全体の無差別コミットを禁止する
- **AND** prompt は worktree を clean にする自律完遂を前提とし、orchestrator に判断を返す逃げ道を設けない

#### Scenario: Cleanup-review output returns single machine-readable verdict

- **GIVEN** cleanup-review operation が完了する
- **WHEN** orchestrator が最終出力を解析する
- **THEN** output には final marker が 1 回だけ含まれる
- **AND** marker は `CLEANUP_REVIEW: CLEAN` のみであり、成功以外の verdict は存在しない
