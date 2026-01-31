## ADDED Requirements
### Requirement: Worktree add failure diagnostics and safe retry

システムは `git worktree add` の失敗時に、stderr から代表的な原因を分類し、診断ログに含めなければならない（MUST）。

分類対象には最低限以下を含めなければならない（MUST）。
- 既存パス（worktree パスが既に存在）
- ブランチ重複（他の worktree でチェックアウト済み）
- 無効な参照（base commit / branch が存在しない）
- 権限エラー

`git worktree add` が既存パス起因で失敗した場合、システムは worktree 一覧に該当パスが存在しないことを確認できたときに限り、`git worktree prune` を実行し、1 回だけ再試行しなければならない（MUST）。

再試行後も失敗した場合、システムは元のエラーと分類結果の両方をログに残さなければならない（MUST）。

#### Scenario: 既存パスの失敗は分類される
- **GIVEN** `git worktree add` が「path already exists」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「既存パス」として分類される
- **AND** 分類結果が診断ログに含まれる

#### Scenario: ブランチ重複の失敗は分類される
- **GIVEN** `git worktree add` が「branch is already checked out」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「ブランチ重複」として分類される
- **AND** 分類結果が診断ログに含まれる

#### Scenario: 無効な参照の失敗は分類される
- **GIVEN** `git worktree add` が「invalid reference」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「無効な参照」として分類される
- **AND** 分類結果が診断ログに含まれる

#### Scenario: 権限エラーの失敗は分類される
- **GIVEN** `git worktree add` が「permission denied」相当の stderr を返す
- **WHEN** worktree 作成に失敗する
- **THEN** 原因は「権限エラー」として分類される
- **AND** 分類結果が診断ログに含まれる

#### Scenario: 既存パスで stale な worktree の場合は prune + 再試行
- **GIVEN** worktree パスが存在するが `git worktree list` に登録されていない
- **AND** `git worktree add` が既存パス起因で失敗する
- **WHEN** worktree 作成が再試行される
- **THEN** `git worktree prune` が実行される
- **AND** `git worktree add` は 1 回だけ再試行される

#### Scenario: 再試行が失敗した場合は元のエラーも保持する
- **GIVEN** `git worktree add` が既存パス起因で失敗する
- **AND** prune 後の再試行も失敗する
- **WHEN** エラーが記録される
- **THEN** 元のエラーと分類結果が両方ログに含まれる
