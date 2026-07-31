## MODIFIED Requirements

### Requirement: Serial Apply Iteration WIP Commits

逐次（非parallel）applyループでは、各イテレーション終了後に作業内容をWIPコミットとして保存しなければならない（MUST）。apply成功・失敗や進捗増加の有無に関わらず、最新状態をスナップショットとして残さなければならない（MUST）。

WIPコミットメッセージは `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})` の形式としなければならない（MUST）。Gitリポジトリで実行中の場合、`git add -A` と `git commit --no-verify --allow-empty` 相当の操作で新規WIPコミットを作成しなければならない（MUST）。既存WIPコミットの `--amend` を使用してはならない（MUST NOT）。

Conflux が所有する WIP スナップショットの `git add -A` または `git commit --no-verify --allow-empty` が、管理対象 worktree の Git directory に解決される `index.lock` を既存ファイルのため作成できず失敗した場合に限り、Conflux は `create_progress_commit` の完全な snapshot sequence を最大3回、固定200ミリ秒間隔、backoffなしで再試行しなければならない（MUST）。Conflux は lock を削除してはならず（MUST NOT）、一般の Git command にこの retry policy を適用してはならない（MUST NOT）。

各 attempt 前に `HEAD_before` を記録し、失敗後の HEAD が前進し、その新HEADの唯一のparentが `HEAD_before` で、subjectが期待するWIP messageと完全一致する場合に限り、その attempt をcommit済みとして扱わなければならない（MUST）。履歴中の同一subjectだけを成功証拠にしてはならない（MUST NOT）。Cancellation は VCS trait の内部ではなく progress-commit orchestration boundary で retryable failure 後、待機前、次 attempt 前に確認しなければならない（MUST）。競合が3回で解消しない場合、および分類条件を満たさない VCS error の場合は、作業内容を保持して診断可能な失敗を返さなければならない（MUST）。

#### Scenario: WIP created after successful apply iteration

- Given: 逐次applyループが実行中である
- When: applyコマンドが正常に完了しイテレーションが終了する
- Then: WIPスナップショットが新規コミットとして作成される

#### Scenario: WIP created after failed apply iteration

- Given: 逐次applyループが実行中である
- When: applyコマンドが失敗してイテレーションが終了する
- Then: 失敗時点の作業内容がWIPスナップショットとして保存される

#### Scenario: WIP created when no progress is made

- Given: 逐次applyループが実行中である
- When: applyコマンドは成功したがタスク進捗が増加しない
- Then: 最新の作業内容を反映したWIPスナップショットが作成される

#### Scenario: Transient index lock clears within retry budget

- **GIVEN** Conflux-owned WIP `git add -A` or WIP commit reports an existing `index.lock` that resolves to the current managed worktree Git directory
- **AND** the lock becomes available before the third total attempt
- **WHEN** Conflux retries the complete progress-commit snapshot sequence after the fixed 200 ms delay
- **THEN** the expected WIP commit is created exactly once
- **AND** staged and unstaged apply output is included in the snapshot
- **AND** Conflux does not delete or bypass the lock

#### Scenario: Ambiguous commit completion does not duplicate WIP

- **GIVEN** a WIP commit attempt captured `HEAD_before` and then reports failure
- **AND** current HEAD advanced to a commit whose only parent is `HEAD_before` and whose subject exactly matches the expected WIP message
- **WHEN** Conflux evaluates whether another attempt is required
- **THEN** Conflux recognizes that attempt as committed
- **AND** no duplicate WIP commit is created
- **AND** a same-subject commit elsewhere in history is not accepted as evidence

#### Scenario: Persistent index lock exhausts retries safely

- **GIVEN** the managed worktree `index.lock` remains unavailable for all three attempts
- **WHEN** Conflux exhausts the third attempt
- **THEN** apply fails with command, working directory, lock contention, and attempt diagnostics
- **AND** the apply output remains preserved in the workspace
- **AND** the lock file is not deleted

#### Scenario: Cancellation stops lock retry

- **GIVEN** Conflux has classified a WIP snapshot lock failure as retryable
- **WHEN** runtime cancellation is observed after that failure, before delay, or before the next attempt
- **THEN** no further snapshot attempt starts
- **AND** the workspace state remains preserved

#### Scenario: Non-lock VCS failure is not retried

- **GIVEN** a WIP snapshot fails because of a permission, identity, configuration, hook, conflict, or other non-lock VCS error
- **WHEN** Conflux classifies the failure
- **THEN** Conflux returns the structured VCS failure without applying the transient lock retry policy
