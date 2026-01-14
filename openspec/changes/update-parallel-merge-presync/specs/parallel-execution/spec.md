# parallel-execution Delta

## MODIFIED Requirements

### Requirement: Git Sequential Merge

Git バックエンド使用時、システムは複数ブランチを逐次マージしなければならない（SHALL）。

逐次マージでは、各 change について以下の順序で統合を試みなければならない（SHALL）。

1. 統合先ブランチ（base）の最新を対象 worktree ブランチへ取り込む（base → worktree）
2. 1 が完了した後、統合先ブランチへ対象 worktree ブランチをマージする（worktree → base）

各マージコミットメッセージは `Merge change: <change_id>` の形式とし、対象ブランチに対応する **OpenSpec の change_id**（`openspec/changes/{change_id}`）を含めなければならない（MUST）。

#### Scenario: Merge change_id は OpenSpec の change_id を使う

- **GIVEN** 逐次マージ対象の worktree ブランチと、それぞれに対応する OpenSpec の change_id が存在する
- **WHEN** `resolve_command` が逐次マージを完了する
- **THEN** 各マージコミットの subject は `Merge change: <change_id>` の形式である
- **AND** `change_id` は `openspec/changes/{change_id}` の ID と一致する

#### Scenario: 事前同期でコンフリクト解消を worktree 側で完結する

- **GIVEN** 対象 worktree ブランチの作成後に、統合先ブランチ（base）が更新されている
- **WHEN** システムが対象 change の逐次マージを開始する
- **THEN** システムはまず base → worktree の取り込みを試みる
- **AND** コンフリクトが発生した場合、コンフリクト解消は対象 worktree の作業コピーで行われる
- **AND** 事前同期が完了した後に worktree → base のマージが行われる
- **AND** 最終統合のマージコミット subject は `Merge change: <change_id>` である
