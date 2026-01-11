# Spec: parallel-execution (Delta)

## MODIFIED Requirements

### Requirement: Periodic Progress Commits

並列実行のapplyループにおいて、各イテレーション終了後に進捗をコミットとして保存する。

#### Scenario: Progress commit created after successful apply

- Given: applyコマンドが正常に完了し、タスク進捗が増加した
- When: イテレーションが終了する
- Then: `WIP: {change_id} ({completed}/{total} tasks)` 形式のコミットが作成される

#### Scenario: No commit when no progress made

- Given: applyコマンドが正常に完了したが、タスク進捗が増加しなかった
- When: イテレーションが終了する
- Then: コミットは作成されない（前回のコミットが維持される）

#### Scenario: jj backend commit handling

- Given: jjバックエンドを使用している
- When: 進捗コミットを作成する
- Then: `jj describe --ignore-working-copy` でコミットメッセージが更新される

#### Scenario: Git backend commit handling

- Given: Gitバックエンドを使用している
- When: 進捗コミットを作成する
- Then: `git add -A && git commit --amend` で変更がコミットされる
