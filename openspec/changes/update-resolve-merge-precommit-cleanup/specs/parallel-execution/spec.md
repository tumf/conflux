## MODIFIED Requirements

### Requirement: Git Conflict Resolution

Git バックエンド使用時、システムは resolve コマンドの再試行時に前回の試行結果と継続理由をプロンプトに含めなければならない（MUST）。

resolve の目標（完了条件）は、少なくとも以下を満たすこととする：

- `git diff --name-only --diff-filter=U` が空である（未解決コンフリクトがない）
- Git マージが完了している（例: `MERGE_HEAD` が存在しない）
- 対象の各 `change_id` について、`Merge change: <change_id>` を含むマージコミットが存在する

resolve のプロンプトには、`--no-verify` を使用してはならない旨を明示しなければならない（MUST）。

resolve の最終マージは `git merge --no-ff --no-commit <branch>` で開始し、コミット前に以下を実行するようプロンプトで指示しなければならない（MUST）：

- `openspec/changes/{change_id}/proposal.md` が存在し、かつ `openspec/changes/archive/` 配下に同一 `change_id` のアーカイブが存在する場合、`openspec/changes/{change_id}` を削除する
- 削除後に `git add -A` を実行し、`git commit -m "Merge change: <change_id>"` で同一マージコミットを作成する

上記の目標が満たされない場合、システムは継続理由を記録し、次回の `resolve_command` プロンプトに含めて再実行しなければならない（SHALL）。

#### Scenario: resolve プロンプトが no-commit と復活削除手順を含む
- **WHEN** システムが resolve プロンプトを生成する
- **THEN** プロンプトに `git merge --no-ff --no-commit <branch>` が含まれる
- **AND** プロンプトに `openspec/changes/{change_id}` の復活検知と削除手順が含まれる

#### Scenario: 復活した changes はマージコミット前に削除される
- **GIVEN** `openspec/changes/{change_id}/proposal.md` が存在する
- **AND** `openspec/changes/archive/` 配下に同一 `change_id` のアーカイブが存在する
- **WHEN** resolve の最終マージ手順を実行する
- **THEN** `openspec/changes/{change_id}` は `git commit -m "Merge change: <change_id>"` の前に削除される
