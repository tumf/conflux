## MODIFIED Requirements

### Requirement: git/sync must only run reconciliation when needed before push

The server git sync workflow MUST determine whether reconciliation is required after refreshing remote state and MUST avoid invoking `resolve_command` when the local branch is already synchronized with the target remote branch.

判定は pull フェーズ完了後に収集した `local_sha_for_push` と `remote_sha_for_push` の比較で行う。pre-pull SHA には依存しない。

`resolve_command` は AI エージェントを起動する高コストな処理であるため、その起動可否は push を試行してから失敗で検知するのではなく、**push 試行前の SHA 比較で事前判断**しなければならない（MUST）。両 SHA が非空かつ一致する場合はエージェント起動と push を省略する。

**Implementation:** `src/server/api/git_sync.rs` の `plan_sync()` (L181-188) と呼び出し元 L491-499。`plan_sync` は `remote_sha_for_push` が空でなく `local_sha_for_push` と一致する場合のみ `should_skip_resolve_and_push = true` を返す。

**推奨設定:**
- トップレベル config に `resolve_command` を必須設定する（`server.resolve_command` は廃止済み。設定すると起動時エラー）。詳細は `src/config/types.rs` L215-216, L457-502。
- `resolve_command` は非ゼロ終了で sync を失敗させるため、冪等かつ失敗時に明確なエラー出力を返すコマンドを指定すること。

本 Requirement は旧 spec 内で重複していた 2 版（post-pull 比較版と pre-pull vs post-pull 比較版）のうち、実装と一致する post-pull 比較版に一本化したものである（pre-pull vs post-pull 比較版は削除）。

#### Scenario: resolve_command invocation is decided before agent startup

**Given** the pull phase has completed
**When** `git/sync` decides whether to invoke `resolve_command`
**Then** it MUST evaluate the SHA comparison rule *before* spawning the agent process
**And** it MUST NOT rely on push rejection or post-hoc failure to trigger `resolve_command`

#### Scenario: local and remote branch tips already match

**Given** the server has completed the pull phase for a project branch
**And** the computed local branch SHA equals the current remote branch SHA
**When** `git/sync` evaluates whether to run pre-push reconciliation
**Then** it MUST skip `resolve_command`
**And** it MUST return a successful sync response without attempting a push

#### Scenario: local and remote branch tips differ

**Given** the server has completed the pull phase for a project branch
**And** the computed local branch SHA differs from the current remote branch SHA
**When** `git/sync` evaluates whether to run pre-push reconciliation
**Then** it MUST run `resolve_command` before attempting push
**And** it MUST fail the sync if `resolve_command` exits non-zero

#### Scenario: remote branch does not yet exist for push comparison

**Given** the server has completed the pull phase for a project branch
**And** the remote SHA for push comparison is empty because the remote branch does not yet exist
**When** `git/sync` evaluates whether to run pre-push reconciliation
**Then** it MUST NOT treat the branch as already synchronized
**And** it MUST continue with the existing resolve-before-push flow

#### Scenario: bare repo is newly cloned (first sync)

**Given** the local bare repo did not exist before this sync invocation and was freshly cloned
**When** `git/sync` evaluates whether to run pre-push reconciliation
**Then** the post-pull `remote_sha_for_push` may be empty or may match `local_sha_for_push` depending on remote state
**And** the skip decision MUST follow the standard rule (skip only when both SHAs are non-empty and equal)

## REMOVED Requirements

### Requirement: git/sync must only run reconciliation when needed before push (pre-pull vs post-pull 比較版)

旧 spec 内に併記されていた、pull フェーズ **前** の local SHA と pull フェーズ **後** の remote SHA を比較するバージョンを削除する。実装 (`src/server/api/git_sync.rs::plan_sync`) は両方とも post-pull の SHA で比較しており、pre-pull SHA には依存しない。本版は実装と不一致のまま spec に残っていた古い記述であり、削除によって canonical spec を実装と一本化する。
