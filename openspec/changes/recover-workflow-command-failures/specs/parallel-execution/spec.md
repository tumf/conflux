## MODIFIED Requirements

### Requirement: AI エージェントクラッシュリカバリー

Apply または Archive コマンドが異常終了（exit code ≠ 0）した場合、システムはコマンドキューの既存 transport retry を適用しなければならない（SHALL）。transport retry が終了しても Apply command が非ゼロの場合、workspace-local completion、cancellation、permission、blocked/rejecting handoff の別 routing が所有しない限り、システムは失敗 attempt を既存 Apply history に記録し、同一 managed workspace 上の次の Apply iteration を実行しなければならない（MUST）。

Apply の全 outer attempt は正式な `max_iterations` 設定を共有しなければならない（MUST）。正の値は command failure、通常実装、task-format repair、escalation、final-commit repair を含む dispatched Apply attempt の総上限であり、`0` は numeric limit を無効化しなければならない（MUST）。Archive command の transport retry は既存 `ARCHIVE_COMMAND_MAX_RETRIES` を使用し、operation-level Archive recovery はこの requirement では追加しない。

#### Scenario: Parallel Apply command failure continues with bounded history

- **GIVEN** parallel mode の Apply command が command queue retry 後も非ゼロで終了する
- **AND** tasks は未完了で cancellation、permission stall、blocked/rejecting handoff、completion-finalized state のいずれも存在しない
- **AND** 正の `max_iterations` budget が残っている
- **WHEN** Apply loop が command result を処理する
- **THEN** exit code、error、bounded stdout tail、bounded stderr tail を Apply history に記録する
- **AND** 同じ managed workspace 上で次の Apply iteration を実行する
- **AND** 次の prompt はその bounded failure context を含む
- **AND** queue state は Applying のままで terminal processing error を生成しない

#### Scenario: Apply command failures exhaust one total iteration budget

- **GIVEN** `max_iterations` が `3` である
- **AND** command queue 内部 retry 後の Apply command が繰り返し非ゼロで終了する
- **WHEN** 3 回の outer Apply attempt が完了する
- **THEN** 4 回目の Apply command は開始しない
- **AND** terminal diagnostic は上限と最新の bounded actionable failure を含む

#### Scenario: Zero leaves Apply iteration count unlimited

- **GIVEN** `max_iterations` が `0` である
- **WHEN** Apply command failure または未完了 progress により複数の outer iteration が必要になる
- **THEN** iteration count のみを理由に Apply loop を停止しない
- **AND** completion、cancellation、stall、permission、blocked/rejecting handoff、または他の owned terminal outcome は引き続き有効である

#### Scenario: Owned Apply outcomes do not become generic crash recovery

- **GIVEN** Apply execution が explicit cancellation、repeated unresolved permission denial、blocked/rejecting handoff、または observed completion 後の orchestrator terminate に到達する
- **WHEN** child status が非ゼロである
- **THEN** 既存の cancellation、permission stall、handoff、または completion-finalized routing を保持する
- **AND** ordinary command-failure iteration として重複 retry しない

## ADDED Requirements

### Requirement: Acceptance command failures MUST use bounded Acceptance-only recovery

Serial および parallel execution は、configured Acceptance command の launch または execution failure を、同一の applied かつ clean な workspace 上で Acceptance だけを再実行する active-run recovery として扱わなければならない（MUST）。初回 failure 後の retry は最大 2 回で、3 回目の連続 command failure 後にのみ terminal error としなければならない（MUST）。

この command-failure counter は missing-verdict または他の protocol correction、explicit `CONTINUE`、canonical FAIL から Apply への repair cycle、および `MAX_ACCEPTANCE_RETRY_CYCLES` から独立しなければならない（MUST）。retry は Apply または cleanup-review を再実行してはならず（MUST NOT）、canonical Acceptance outcome は counter を reset して既存 routing を保持しなければならない（MUST）。

#### Scenario: Acceptance command recovers without rerunning Apply

- **GIVEN** applied かつ clean な workspace の Acceptance command が command queue retry 後に失敗する
- **AND** dedicated command-failure retry budget が残っている
- **WHEN** serial または parallel runtime が failure を処理する
- **THEN** latest bounded error、exit code、stdout tail、stderr tail を次の Acceptance prompt に渡す
- **AND** normal configured Acceptance command だけを再実行する
- **AND** Apply と cleanup-review の invocation count は増加しない

#### Scenario: Acceptance command retry budgets remain independent

- **GIVEN** Acceptance command failure recovery が active である
- **WHEN** runtime が次の Acceptance invocation を開始する
- **THEN** missing-verdict/protocol、explicit-CONTINUE、FAIL-to-Apply cycle の counters を消費しない
- **AND** FAIL findings を tasks に追加しない
- **AND** outer Apply/Acceptance `cycle_count` を command failure のみを理由に増加させない

#### Scenario: Canonical outcome resets Acceptance command recovery

- **GIVEN** one or more Acceptance command failures were followed by a successful command invocation
- **WHEN** that invocation emits canonical PASS, FAIL, CONTINUE, validated stalled, or permission-stalled output
- **THEN** consecutive command-failure state resets
- **AND** the canonical outcome follows its existing routing semantics

#### Scenario: Acceptance command recovery exhausts after three failures

- **GIVEN** initial Acceptance invocation and two corrective invocations all fail at command level
- **WHEN** runtime processes the third consecutive failure
- **THEN** it emits one terminal Acceptance command failure containing the attempt count and latest bounded diagnostics
- **AND** no fourth command-failure invocation starts
- **AND** the managed workspace remains available for explicit retry

#### Scenario: Cancellation and permission stall bypass command recovery

- **GIVEN** Acceptance is cancelled or produces the existing classified permission-stall outcome
- **WHEN** runtime routes the result
- **THEN** it does not classify the result as an ordinary command failure
- **AND** it starts no Acceptance command-failure retry

#### Scenario: Restart derives Acceptance from workspace evidence

- **GIVEN** Acceptance command recovery was active before process termination
- **AND** the workspace remains applied, clean, and unarchived
- **WHEN** Conflux restarts
- **THEN** it runs Acceptance from workspace and Git evidence with a fresh active-run budget
- **AND** it does not require a report, retry checkpoint, provider session, external job identifier, cache, or prior log

### Requirement: Managed worktree apply MUST run post-apply cleanup review before acceptance handoff

Parallel mode で Conflux-managed isolated worktree 上の Apply が task completion に到達したあと、worktree が dirty のままなら、システムは Acceptance 開始前に post-apply cleanup-review を実行しなければならない（MUST）。cleanup-review は初回 operation attempt に加えて最大 2 回の corrective attempt を許可し、各 attempt の command queue transport retry とは別に数えなければならない（MUST）。

各 corrective prompt は latest failure kind、利用可能な exit code、bounded stdout/stderr、standalone marker count、および fresh bounded porcelain status を含まなければならない（MUST）。成功には acceptable command completion、`CLEANUP_REVIEW: CLEAN` standalone marker が exactly once、および fresh repository query による clean worktree のすべてが必要である（MUST）。cleanup-review が成功するまで Acceptance に進めてはならない（MUST NOT）。

Cleanup retry control は active-run memory にのみ保持し、通常 Apply loop へ戻ってはならない（MUST NOT）。cancellation は active child を terminate して retry を停止し、classified permission denial は既存 permission/stall routing を保持しなければならない（MUST）。3 回の operation attempt が失敗した場合のみ、latest bounded diagnosis を伴う terminal error とし、managed workspace evidence を保持しなければならない（MUST）。

Apply runtime が tasks.md 上の完了条件、または `REJECTED.md` による apply-blocked handoff を既に観測した run では、agent process やその子プロセスが stdout/stderr を保持したまま自然終了しなくても、システムは有限な grace period 後に当該 process group を terminate して handoff 判定へ進まなければならない（MUST）。この早期 terminate は完了条件を観測済みの場合にのみ成功相当として扱われなければならない（MUST）。

#### Scenario: Dirty managed worktree recovers on corrective cleanup attempt

- **GIVEN** task-complete managed worktree is dirty after Apply
- **AND** initial cleanup-review command fails, omits or duplicates the marker, or leaves the worktree dirty
- **AND** corrective budget remains
- **WHEN** orchestrator starts the next cleanup-review operation attempt
- **THEN** the prompt includes only the latest bounded structured failure and current porcelain evidence
- **AND** cleanup-review inspects and repairs the actual managed workspace without returning to ordinary Apply
- **AND** Acceptance remains blocked until the full success gate passes

#### Scenario: Marker without clean repository is not success

- **GIVEN** cleanup-review output contains exactly one standalone `CLEANUP_REVIEW: CLEAN`
- **AND** fresh repository status remains dirty or status inspection fails
- **WHEN** orchestrator validates the attempt
- **THEN** it classifies the attempt as cleanup failure
- **AND** it does not start Acceptance

#### Scenario: Clean repository without marker is not success

- **GIVEN** fresh repository status is clean
- **AND** cleanup-review output has zero or multiple standalone clean markers
- **WHEN** orchestrator validates the attempt
- **THEN** it classifies the attempt as cleanup protocol failure
- **AND** it does not start Acceptance

#### Scenario: Cleanup-review succeeds only after ordered dual proof

- **GIVEN** cleanup-review command completes acceptably
- **AND** output contains exactly one standalone `CLEANUP_REVIEW: CLEAN`
- **AND** a subsequent fresh repository query proves no tracked, staged, unstaged, or untracked changes
- **WHEN** orchestrator completes validation
- **THEN** it marks cleanup handoff successful
- **AND** only then may it start Acceptance

#### Scenario: Cleanup-review exhaustion preserves workspace

- **GIVEN** initial cleanup-review and two corrective attempts fail
- **WHEN** orchestrator processes the third failure
- **THEN** it emits terminal cleanup-review error with attempt count and latest bounded diagnosis
- **AND** it starts no fourth operation attempt
- **AND** it preserves the managed worktree and its repository evidence for explicit retry

#### Scenario: Cleanup-review cancellation terminates without retry

- **GIVEN** per-change cancellation occurs while cleanup-review streams output or waits for child status
- **WHEN** orchestrator observes cancellation
- **THEN** it terminates the active process group through managed cleanup
- **AND** it starts no corrective attempt
- **AND** existing intentional-stop routing remains authoritative

#### Scenario: Cleanup-review restart uses workspace evidence

- **GIVEN** cleanup-review correction was active before process termination
- **WHEN** Conflux restarts with the same task-complete dirty workspace
- **THEN** it derives cleanup-review as the next action from workspace and Git state
- **AND** it does not require a cleanup report, retry marker, prior logs, or another out-of-worktree workflow-control input
