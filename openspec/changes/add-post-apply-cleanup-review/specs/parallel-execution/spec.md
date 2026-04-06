## ADDED Requirements

### Requirement: Managed worktree apply MUST run post-apply cleanup review before acceptance handoff

parallel mode で Conflux-managed isolated worktree 上の apply がタスク完了に到達したあと、worktree が dirty のままなら、システムは acceptance 開始前に post-apply cleanup review を実行しなければならない（MUST）。cleanup review が成功するまで acceptance に進めてはならない（MUST NOT）。

#### Scenario: Dirty managed worktree triggers cleanup review after apply completion

- **GIVEN** parallel mode の apply が managed git worktree 上で実行されている
- **AND** apply loop が tasks.md 上の完了条件を満たして終了した
- **AND** worktree に未コミット変更または未追跡ファイルが残っている
- **WHEN** orchestrator が apply 完了 handoff を判定する
- **THEN** orchestrator は acceptance を開始せず cleanup review operation を起動する
- **AND** cleanup review 成功後にのみ apply 完了を確定して acceptance に進む

#### Scenario: Clean managed worktree skips cleanup review

- **GIVEN** parallel mode の apply が managed git worktree 上で実行されている
- **AND** apply loop が tasks.md 上の完了条件を満たして終了した
- **AND** worktree が clean である
- **WHEN** orchestrator が apply 完了 handoff を判定する
- **THEN** cleanup review は不要である
- **AND** orchestrator は従来どおり apply 完了を確定して acceptance に進む

#### Scenario: Blocked cleanup review prevents acceptance handoff

- **GIVEN** parallel mode の managed worktree apply が task-complete だが dirty である
- **AND** cleanup review operation が safe handoff を成立させられない
- **WHEN** orchestrator が cleanup review verdict を処理する
- **THEN** change は acceptance や archive に進まない
- **AND** current run では apply 側の失敗として停止する
- **AND** workspace は follow-up 用に保持される
