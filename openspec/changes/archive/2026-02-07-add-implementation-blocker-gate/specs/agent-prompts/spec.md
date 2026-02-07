## ADDED Requirements

### Requirement: Apply prompt MUST escalate implementation blockers
apply プロンプトは、仕様矛盾や非モック可能な外部制限により実装が不可能と判断した場合、Implementation Blocker を記録してエスカレーションしなければならない（MUST）。

Implementation Blocker の記録は以下を満たさなければならない（MUST）。
- `openspec/changes/{change_id}/tasks.md` に `## Implementation Blocker #<n>` セクションを追加する
- セクション内に「カテゴリ」「根拠（ファイルパス/ログ）」「影響範囲」「解除アクション」を明記する
- セクション内の箇条書きにチェックボックスを付けてはならない（MUST NOT）
- stdout に `IMPLEMENTATION_BLOCKER:` ブロックを出力し、tasks.md と同じ内容を含める

#### Scenario: apply が実装不可を検知して blocker を記録する
- **GIVEN** apply が仕様矛盾または非モック可能な外部制限により実装不可と判断する
- **WHEN** apply がエスカレーションを行う
- **THEN** tasks.md に `## Implementation Blocker #<n>` セクションが追加される
- **AND** セクション内にカテゴリ・根拠・影響範囲・解除アクションが記載される
- **AND** stdout に `IMPLEMENTATION_BLOCKER:` ブロックが出力される

### Requirement: Acceptance prompt MUST evaluate implementation blockers
acceptance プロンプトは Implementation Blocker を審査し、妥当と判断した場合は `ACCEPTANCE: BLOCKED` を出力しなければならない（MUST）。

acceptance は以下を満たさなければならない（MUST）。
- `Implementation Blocker` の内容が不十分または誤りの場合は `ACCEPTANCE: FAIL` を出力し、follow-up タスクを tasks.md に追加する
- `ACCEPTANCE: BLOCKED` の場合は blocker の概要を簡潔に出力する

#### Scenario: acceptance が blocker を承認して BLOCKED を返す
- **GIVEN** tasks.md に妥当な Implementation Blocker が記録されている
- **WHEN** acceptance が blocker を評価する
- **THEN** acceptance は `ACCEPTANCE: BLOCKED` を出力する
