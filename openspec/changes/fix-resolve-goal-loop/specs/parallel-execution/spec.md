## MODIFIED Requirements

### Requirement: Git Conflict Resolution

Git バックエンド使用時、システムは Git コンフリクトマーカーを含む解決プロンプトを提供しなければならない（SHALL）。

さらに、システムは `resolve_command` の完了を「コマンドが成功終了したこと」ではなく「Resolve の目標（完了条件）が満たされたこと」で判定しなければならない（MUST）。

Resolve の目標（完了条件）は、少なくとも以下を満たすこととする（MUST）。

- `git diff --name-only --diff-filter=U` が空である（未解決コンフリクトがない）
- Git マージが完了している（例: `MERGE_HEAD` が存在しない／`git status` が still merging を示さない）
- 対象の各 `change_id` について、`Merge change: <change_id>` を含むマージコミットが存在する

上記の目標が満たされない場合、システムは `resolve_command` を再実行して収束させなければならない（SHALL）。

#### Scenario: コンフリクト解消後もマージ未完了なら Resolve は継続する

- **GIVEN** `git diff --name-only --diff-filter=U` が空である
- **AND** Git がマージ進行中状態である（例: `MERGE_HEAD` が存在する）
- **WHEN** `resolve_command` が成功終了する
- **THEN** システムは Resolve を成功扱いせず、`resolve_command` を再実行する
- **AND** 最大リトライ回数を超えても目標が満たされない場合、エラーとして扱う

#### Scenario: マージコミットが不足している場合は Resolve を継続する

- **GIVEN** 対象の `change_id` のうち一部について `Merge change: <change_id>` を含むマージコミットが存在しない
- **WHEN** `resolve_command` が成功終了する
- **THEN** システムは Resolve を成功扱いせず、`resolve_command` を再実行する

#### Scenario: `approved` だけが残った change ディレクトリは削除して完了する

- **GIVEN** archive により `openspec/changes/{change_id}` は存在しないことが期待される
- **AND** しかし `openspec/changes/{change_id}` が存在し、その内容が `approved` ファイルのみである
- **WHEN** Resolve の目標判定が行われる
- **THEN** システムは `openspec/changes/{change_id}` をディレクトリごと削除して完了とする
