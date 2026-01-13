## MODIFIED Requirements
### Requirement: Git Conflict Resolution

Git バックエンド使用時、システムは Git コンフリクトマーカーを含む解決プロンプトを提供しなければならない（SHALL）。
さらに、コンフリクト解決後はマージが完了するまで再試行ループを実行しなければならない（SHALL）。

#### Scenario: Git conflict resolution prompt

- **WHEN** Git マージでコンフリクトが発生する
- **THEN** AgentRunner に渡されるプロンプトに以下が含まれる:
  - "This project uses Git for version control, not jj."
  - コンフリクトファイルのリスト
  - Git コンフリクトマーカーの説明（`<<<<<<<`, `=======`, `>>>>>>>`）
  - `git status` の出力
  - 解決後の手順

#### Scenario: Resolution success

- **WHEN** AgentRunner がコンフリクトを解決する
- **AND** `git diff --name-only --diff-filter=U` が空を返す
- **THEN** システムは対象リビジョンのマージを再試行する
- **AND** マージが成功するまで resolve → merge を繰り返す
- **AND** 次のブランチのマージに進む

#### Scenario: Resolution failure after retries

- **WHEN** 最大リトライ回数（デフォルト3回）を超えてもマージが完了しない
- **THEN** エラーメッセージが表示される
- **AND** ワークスペースは保持される（手動検査用）
