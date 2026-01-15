# Parallel Execution Spec Deltas

## MODIFIED Requirements

### Requirement: 未コミット change の tasks 読み込みを行わない

並列モードは、**実行対象の判定**にコミットツリーを利用し、未コミット change を実行対象としてはならない（SHALL NOT）。

ただし、**進捗表示**については、worktree 内の未コミット `tasks.md` が存在する場合、それを優先的に読み取り、即座にユーザーに反映しなければならない（SHALL）。

#### Scenario: 未コミット change は実行対象外

- **GIVEN** `HEAD` のコミットツリーに存在しない未コミット change がある
- **WHEN** 並列モードの対象判定が行われる
- **THEN** その change は実行対象から除外される
- **AND** 除外された change ID が警告ログに記録される

#### Scenario: Worktree の未コミット tasks.md から進捗を読む

- **GIVEN** 並列実行中の change に対応する worktree が存在する
- **AND** worktree 内の `openspec/changes/{change_id}/tasks.md` が更新されている（未コミット）
- **WHEN** TUI の auto-refresh が実行される
- **THEN** システムは worktree 内の tasks.md を読み取る
- **AND** ベースツリーの tasks.md よりも worktree の内容が優先される
- **AND** TUI に即座に最新の進捗が表示される

#### Scenario: Worktree が存在しない場合のフォールバック

- **GIVEN** change に対応する worktree が存在しない
- **WHEN** 進捗の取得が試みられる
- **THEN** システムはベースツリーの `openspec/changes/{change_id}/tasks.md` から進捗を読み取る
- **AND** エラーは発生しない

#### Scenario: Worktree 読み取りエラー時の処理

- **GIVEN** worktree は存在するが tasks.md の読み取りに失敗する
- **WHEN** 進捗の取得が試みられる
- **THEN** システムは warning log を記録する
- **AND** ベースツリーから進捗を読み取る（silent fallback）
- **AND** TUI の表示には影響しない
