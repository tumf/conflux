## MODIFIED Requirements

### Requirement: Managed worktree apply MUST run post-apply cleanup review before acceptance handoff

parallel mode で Conflux-managed isolated worktree 上の apply がタスク完了に到達したあと、worktree が dirty のままなら、システムは acceptance 開始前に post-apply cleanup review を実行しなければならない（MUST）。cleanup review が成功するまで acceptance に進めてはならない（MUST NOT）。

apply runtime が tasks.md 上の完了条件、または `REJECTED.md` による apply-blocked handoff を既に観測した run では、agent process やその子プロセスが stdout/stderr を保持したまま自然終了しなくても、システムは有限な grace period 後に当該 process group を terminate して handoff 判定へ進まなければならない（MUST）。この早期 terminate は完了条件を観測済みの場合にのみ成功相当として扱われなければならない（MUST）。

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

#### Scenario: Completion grace period terminates stale apply agent after tasks complete

- **GIVEN** parallel mode の apply command が tasks.md 上の完了条件を満たす変更を書き込む
- **AND** その後 agent process または子プロセスが stdout/stderr pipe を保持したまま居残り、自然終了しない
- **WHEN** orchestrator が apply 実行中に task completion を観測する
- **THEN** orchestrator は apply completion grace period を開始する
- **AND** grace period が満了しても process が終了しない場合は process group を terminate する
- **AND** 収集済みの workspace 状態に基づいて apply 完了 handoff を続行し acceptance に進む

#### Scenario: Completion grace period terminates stale apply agent after blocked handoff

- **GIVEN** parallel mode の apply command が worktree に `openspec/changes/{change_id}/REJECTED.md` を生成する
- **AND** その後 agent process または子プロセスが自然終了しない
- **WHEN** orchestrator が apply-blocked handoff を観測する
- **THEN** orchestrator は apply completion grace period を開始する
- **AND** grace period 満了後も child が残っていれば terminate する
- **AND** apply loop は rejecting review handoff として有限時間で終了する

#### Scenario: Incomplete apply does not get success-equivalent terminate treatment

- **GIVEN** parallel mode の apply command が tasks.md を未完了のままにしている
- **AND** `REJECTED.md` も生成していない
- **WHEN** agent process が終了せず inactivity timeout や terminate 対象になる
- **THEN** orchestrator はその run を apply 完了として扱ってはならない（MUST NOT）
- **AND** acceptance handoff を開始してはならない（MUST NOT）
- **AND** 従来どおり failure/retry/stall policy に従って扱う
