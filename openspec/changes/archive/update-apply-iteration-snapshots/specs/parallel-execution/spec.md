## MODIFIED Requirements

### Requirement: Periodic Progress Commits

並列実行の apply ループにおいて、各イテレーション終了後に作業内容をスナップショットとして保存しなければならない（MUST）。進捗が増加しない場合でも、最新の作業内容を WIP コミットとして残さなければならない（MUST）。

WIP コミットメッセージは `WIP: {change_id} ({completed}/{total} tasks, apply#{iteration})` の形式としなければならない（MUST）。

#### Scenario: Progress commit created after each successful apply

- Given: apply コマンドが正常に完了した
- When: イテレーションが終了する
- Then: WIP スナップショットが作成される

#### Scenario: Snapshot created even when no progress made

- Given: apply コマンドが正常に完了したが、タスク進捗が増加しなかった
- When: イテレーションが終了する
- Then: 最新の作業内容を反映した WIP スナップショットが作成される

#### Scenario: WIP message includes iteration index

- Given: WIP スナップショットを作成する
- When: コミットメッセージを設定する
- Then: メッセージに `apply#{iteration}` が含まれる

#### Scenario: jj backend snapshot handling

- Given: jj バックエンドを使用している
- When: WIP スナップショットを作成する
- Then: working-copy の内容がスナップショットされ、WIP メッセージが設定される

#### Scenario: Git backend snapshot handling

- Given: Git バックエンドを使用している
- When: WIP スナップショットを作成する
- Then: `git add -A` と `git commit --allow-empty` 相当の操作で WIP コミットが作成される

## ADDED Requirements

### Requirement: Final Apply Squash

すべての apply イテレーションが成功した場合、システムは WIP スナップショットを単一の `Apply: {change_id} (apply#{final_iteration})` コミットに squash しなければならない（MUST）。apply が失敗した場合は squash を行ってはならない（MUST NOT）。

#### Scenario: Successful apply squashes WIP commits

- Given: apply ループが成功で終了した
- When: 最終処理が実行される
- Then: WIP コミットが 1 つの Apply コミットに統合される

#### Scenario: Apply commit includes final iteration index

- Given: Apply コミットを作成する
- When: コミットメッセージを設定する
- Then: `apply#{final_iteration}` が含まれる

#### Scenario: Failed apply preserves WIP commits

- Given: apply ループが失敗で終了した
- When: 終了処理が行われる
- Then: WIP コミットは保持され、squash は実行されない

#### Scenario: jj backend squash handling

- Given: jj バックエンドを使用している
- When: Apply コミットを作成する
- Then: `jj squash` 相当で WIP が統合される

#### Scenario: Git backend squash handling

- Given: Git バックエンドを使用している
- When: Apply コミットを作成する
- Then: `git reset --soft` と `git commit` 相当で WIP が統合される
