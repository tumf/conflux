# configuration Specification

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

#### Scenario: Apply command prompt structure

- **GIVEN** `apply_command` が `"agent apply {change_id} {prompt}"` に設定されている
- **AND** `apply_prompt` が `"Focus on implementation."` に設定されている
- **WHEN** change `add-feature` を apply する
- **THEN** `{prompt}` は `"Focus on implementation.\n\nRemove out-of-scope tasks. Remove tasks that wait for or require user action."` に展開される
- **AND** 未完了タスクを実行可能に保つための指示が含まれる
- **AND** `(future work)` をチェックリストから外すための指示が含まれる

#### Scenario: Apply command with empty user prompt

- **GIVEN** `apply_command` が `"agent apply {change_id} {prompt}"` に設定されている
- **AND** `apply_prompt` が空、または未設定である
- **WHEN** change `add-feature` を apply する
- **THEN** `{prompt}` はハードコードシステムプロンプトのみを展開する

#### Scenario: Apply command with history context

- **GIVEN** `apply_command` が `"agent apply {change_id} {prompt}"` に設定されている
- **AND** `apply_prompt` が `"Focus on implementation."` に設定されている
- **AND** 以前の apply 失敗履歴が存在する
- **WHEN** change `add-feature` を apply する
- **THEN** `{prompt}` はユーザープロンプト + システムプロンプト + 履歴コンテキストに展開される

#### Scenario: Archive command unchanged

- **GIVEN** `archive_command` が `"agent archive {change_id} {prompt}"` に設定されている
- **AND** `archive_prompt` が `"Verify completion."` に設定されている
- **WHEN** change `add-feature` を archive する
- **THEN** `{prompt}` は `archive_prompt` のみに展開される（archive にはハードコードシステムプロンプトを追加しない）
