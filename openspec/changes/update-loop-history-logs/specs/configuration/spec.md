## MODIFIED Requirements
### Requirement: System Prompt for Apply and Archive Commands

オーケストレーターは `apply_command` と `archive_command` の両方で `{prompt}` プレースホルダーをサポートし、システム提供の指示をエージェントコマンドへ注入できなければならない（SHALL）。

オーケストレーターは `apply_prompt` と `archive_prompt` の設定項目を提供し、ユーザーがカスタムのプロンプト値を定義できなければならない（SHALL）。

オーケストレーターは apply コマンド向けに、`apply_prompt` の直後に必ず付与されるハードコードシステムプロンプトを含めなければならない（SHALL）。このシステムプロンプトはタスク管理の必須ルールを強制し、ユーザー設定で無効化できない。

apply コマンドの `{prompt}` は `apply_prompt` + ハードコードシステムプロンプト + 履歴コンテキスト（存在する場合）を改行で連結したものとして展開されなければならない（SHALL）。

ハードコードシステムプロンプトには以下の指示が含まれなければならない（SHALL）。
- "Remove out-of-scope tasks."
- "Remove tasks that wait for or require user action."
- 未完了タスクは必ず実行可能であることを求める指示
- 実行不能タスクを具体コマンド + 合格基準を持つ実行可能タスクへ書き換える指示
- 人間判断や外部アクションが必須な場合のみ `(future work)` を付けて `Future work` セクションへ移動し、チェックボックスを外す指示
- apply が成功しても未完了タスクが残る状態を許容せず、タスク正規化を優先する指示

archive コマンドの `{prompt}` は `archive_prompt` のみに展開される（SHALL）。archive にはハードコードシステムプロンプトを追加してはならない（MUST NOT）。

#### Scenario: Archive command uses configured archive_prompt only
- **GIVEN** `archive_command` が `"agent archive {change_id} {prompt}"` に設定されている
- **AND** `archive_prompt` が空、または未設定である
- **WHEN** change `add-feature` を archive する
- **THEN** `{prompt}` は空文字（または空の値）として展開される
- **AND** system-context の既定文言は含まれない
