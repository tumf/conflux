## ADDED Requirements
### Requirement: Acceptance prompt MUST support sub-agent parallel verification with a single final verdict
acceptance プロンプトは、独立した検証項目をサブエージェントに分割して並列実行し、親エージェントが統合して最終判定を 1 回だけ出力する手順を含めなければならない（MUST）。サブエージェントは `ACCEPTANCE:` を出力してはならない（MUST NOT）。サブエージェントの出力は親が統合可能な構造（例: JSON または見出し + 根拠の箇条書き）であることを要求しなければならない（MUST）。

#### Scenario: サブエージェントの結果を統合して 1 回だけ判定する
- **GIVEN** acceptance プロンプトが生成される
- **WHEN** サブエージェント分割が可能な環境で acceptance を実行する
- **THEN** 親エージェントのみが `ACCEPTANCE:` を 1 回だけ出力する
- **AND** サブエージェントは構造化された結果のみを返す

### Requirement: Acceptance prompt MUST enforce the same scope constraints for sub-agents
acceptance プロンプトは、サブエージェントにも change_id と paths によるスコープ制約を適用し、指定された change 以外の `openspec/changes/**` をレビューしないよう明示しなければならない（MUST）。

#### Scenario: サブエージェントが指定 change のみをレビューする
- **GIVEN** acceptance プロンプトが change_id と paths を提供している
- **WHEN** サブエージェントが検証を実行する
- **THEN** 指定された change 以外のファイルをレビューしない

### Requirement: Acceptance prompt MUST define a sequential fallback when sub-agents are unavailable
acceptance プロンプトは、サブエージェントが利用できない場合に同等のチェックを逐次で実行するフォールバック手順を含めなければならない（MUST）。

#### Scenario: サブエージェントが利用できない場合の逐次実行
- **GIVEN** サブエージェントが利用できない環境で acceptance を実行する
- **WHEN** acceptance プロンプトに従って検証を開始する
- **THEN** 同等のチェックを逐次で完了する手順が提示される
