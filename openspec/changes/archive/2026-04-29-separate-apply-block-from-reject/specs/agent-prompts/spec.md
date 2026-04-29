## MODIFIED Requirements

### Requirement: Apply prompt MUST escalate implementation blockers

apply プロンプトは、仕様矛盾や非モック可能な外部制限により実装が不可能と判断した場合、Implementation Blocker を記録してエスカレーションしなければならない（MUST）。

Implementation Blocker の記録は以下を満たさなければならない（MUST）。
- `openspec/changes/{change_id}/tasks.md` に `## Implementation Blocker #<n>` セクションを追加する
- セクション内に「カテゴリ」「根拠（ファイルパス/ログ）」「影響範囲」「解除アクション」を明記する
- セクション内の箇条書きにチェックボックスを付けてはならない（MUST NOT）
- stdout に `IMPLEMENTATION_BLOCKER:` ブロックを出力し、tasks.md と同じ内容を含める
- recoverable blocker では terminal rejection artifact を生成せず、machine-readable apply outcome として `BLOCKED` を返す
- `REJECTED.md` を生成してよいのは、change 全体の reject 提案として recovery より closure が妥当である理由を apply が明示できる場合に限る

#### Scenario: apply が recoverable blocker を BLOCKED outcome として記録する
- **GIVEN** apply が仕様矛盾、fixture 不足、追加情報待ち、または依存未解消により現時点では実装を進められない
- **AND** blocker section に解除条件を書ける
- **WHEN** apply がエスカレーションを行う
- **THEN** tasks.md に `## Implementation Blocker #<n>` セクションが追加される
- **AND** stdout に `IMPLEMENTATION_BLOCKER:` ブロックが出力される
- **AND** apply outcome は `BLOCKED` として報告される
- **AND** worktree-local `REJECTED.md` は生成されない

#### Scenario: apply が terminal rejection proposal を明示的に区別する
- **GIVEN** apply が proposal の前提破綻や superseded 状態により change 全体を閉じるべきと判断する
- **WHEN** apply が rejection proposal を出す
- **THEN** stdout には recoverable blocker と区別された rejection proposal outcome が出力される
- **AND** worktree-local `REJECTED.md` 生成はこの outcome に限定される

### Requirement: Acceptance prompt MUST evaluate implementation blockers

acceptance プロンプトは Implementation Blocker を審査し、妥当と判断した場合は `ACCEPTANCE: BLOCKED` を出力しなければならない（MUST）。

acceptance は以下を満たさなければならない（MUST）。
- `Implementation Blocker` の内容が不十分または誤りの場合は `ACCEPTANCE: FAIL` を出力し、follow-up タスクを tasks.md に追加する
- `ACCEPTANCE: BLOCKED` の場合は blocker の概要を簡潔に出力する
- apply-generated recoverable blocker を審査するレビュー経路では、「change を reject するか」と「change を blocked のまま保留するか」を区別できなければならない

#### Scenario: rejecting review が reject proposal を却下しつつ blocked 保留を要求する
- **GIVEN** rejecting review が apply-generated rejection proposal を評価している
- **AND** reviewer は change 全体の reject には同意しない
- **AND** 追加情報、仕様整理、fixture 再設計、または依存解消がないと apply を再開しても同じ blocker が再発すると判断する
- **WHEN** reviewer が machine-readable verdict を返す
- **THEN** verdict は immediate apply resume とは区別された blocked 保留 outcome になる
- **AND** runtime は change を `Blocked` へ送る前提でその verdict を扱う
