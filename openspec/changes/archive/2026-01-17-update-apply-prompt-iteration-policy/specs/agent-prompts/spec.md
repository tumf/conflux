## MODIFIED Requirements

### Requirement: Apply system prompt MUST enforce non-interactive iteration

apply system prompt（`APPLY_SYSTEM_PROMPT`）は、ユーザーへの質問ができない運用前提の下で、MaxIteration まで最善を尽くして反復実行を継続することを明示しなければならない（MUST）。

#### Scenario: 質問せずに反復を継続する

- **GIVEN** apply 実行中に不確実な判断が発生する
- **WHEN** apply エージェントが tasks を処理する
- **THEN** ユーザーへの質問を行わずに最善の判断で進行する
- **AND** MaxIteration に到達するまで反復を継続する

### Requirement: Future Work 制限の厳格化

apply system prompt は、難易度・回帰リスク・追加テストの必要性を理由にタスクを Future Work に移してはならないことを明示しなければならない（MUST NOT）。

#### Scenario: 既定以外の Future Work 禁止

- **GIVEN** tasks に難易度が高い項目が含まれる
- **WHEN** apply エージェントが実装手順を判断する
- **THEN** その理由だけで Future Work に移さない
- **AND** 既に `(future work)` と明記されているタスクのみを Future Work として扱う
