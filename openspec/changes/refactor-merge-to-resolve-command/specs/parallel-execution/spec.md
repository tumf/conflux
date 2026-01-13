## MODIFIED Requirements

### Requirement: Git Sequential Merge

Git バックエンド使用時、システムは複数ブランチを逐次マージしなければならない（SHALL）。

ただし、マージの書き込み系操作（`git merge` / `git add` / `git commit`）はオーケストレータが直接実行するのではなく、`resolve_command` に委譲しなければならない（SHALL）。

各ブランチのマージは、fast-forward 可能な場合でもマージコミットを作成しなければならない（SHALL）。

各マージコミットメッセージには、対象ブランチに対応する `change_id` を含めなければならない（MUST）。

#### Scenario: Merge single branch

- **WHEN** 1つのワークスペースブランチをマージする
- **THEN** システムは `resolve_command` を実行し、ターゲットブランチ上で当該ブランチのマージコミットが作成される
- **AND** マージコミットメッセージには対象 `change_id` が含まれる

#### Scenario: Merge multiple branches sequentially

- **WHEN** 複数のワークスペースブランチをマージする
- **THEN** システムは `resolve_command` を実行し、各ブランチが1つずつ逐次マージされる
- **AND** マージ順序はワークスペース作成順である
- **AND** 各ブランチのマージコミットメッセージには対応する `change_id` が含まれる

#### Scenario: pre-commit がファイルを修正してコミットを中断する

- **GIVEN** `git commit` 実行時に pre-commit がファイルを修正し、コミットが中断される
- **WHEN** `resolve_command` がマージコミット作成を試みる
- **THEN** `resolve_command` は修正内容を再ステージし、再度コミットを実行してマージを完了させる

### Requirement: Git Conflict Resolution

Git バックエンド使用時、システムは Git コンフリクトマーカーを含む解決プロンプトを提供しなければならない（SHALL）。
さらに、コンフリクト解決後はマージが完了するまでの手順（再ステージ・再コミットを含む）を `resolve_command` が実行しなければならない（SHALL）。

#### Scenario: Git conflict resolution prompt

- **WHEN** Git マージでコンフリクトが発生する、または発生しうるマージを実行する
- **THEN** `resolve_command` に渡されるプロンプトに以下が含まれる:
  - "This project uses Git for version control, not jj."
  - マージ対象ブランチの順序付きリスト
  - 各ブランチに対応する `change_id`
  - Git コンフリクトマーカーの説明（`<<<<<<<`, `=======`, `>>>>>>>`）
  - `git status` の出力
  - 解決後の手順（`git add` / `git commit`）
  - マージコミットメッセージに `change_id` を含める規約

#### Scenario: Resolution success

- **WHEN** `resolve_command` がコンフリクトを解決し、マージコミット作成まで完了する
- **AND** `git diff --name-only --diff-filter=U` が空を返す
- **THEN** システムは次のブランチのマージに進む

#### Scenario: Resolution failure after retries

- **WHEN** 最大リトライ回数（デフォルト3回）を超えてもマージが完了しない
- **THEN** エラーメッセージが表示される
- **AND** ワークスペースは保持される（手動検査用）
