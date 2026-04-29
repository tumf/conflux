## MODIFIED Requirements

### Requirement: Acceptance prompt MUST evaluate implementation blockers

acceptance プロンプトは Implementation Blocker を審査し、妥当と判断した場合は canonical verdict として `ACCEPTANCE: GATED` / `{"acceptance":"gated"}` を出力しなければならない（MUST）。

acceptance は以下を満たさなければならない（MUST）。
- `Implementation Blocker` の内容が不十分または誤りの場合は `ACCEPTANCE: FAIL` を出力し、follow-up タスクを tasks.md に追加する
- `ACCEPTANCE: GATED` の場合は blocker の概要を簡潔に出力する
- repo 内編集・テスト更新・spec/tasks/proposal 修正など、AI がこの repository 内の変更だけで自律的に解決できる問題は `FAIL` として返さなければならない（MUST）。
- 人判断待ち、repo 外の設定変更、外部依存の解消待ち、追加情報待ち、または apply を再実行しても repository 変更だけでは解決不能な blocker は `GATED` として返さなければならない（MUST）。
- `GATED` を返さず `FAIL` を返した場合、その finding は apply へ戻って repository 作業を行うことで解消可能であることを意味しなければならない（MUST）。
- apply-generated recoverable blocker を審査するレビュー経路では、「change を reject するか」と「change を gated hold のまま保留するか」を区別できなければならない
- 互換期間中に旧 `blocked` acceptance verdict を runtime が受理できても、新規 prompt contract は `gated` を canonical term として使わなければならない（MUST）

#### Scenario: acceptance emits gated for a valid implementation blocker
- **GIVEN** acceptance が妥当な Implementation Blocker を確認した
- **AND** その blocker は repository 内編集だけでは解決できない
- **WHEN** reviewer が machine-readable verdict を返す
- **THEN** verdict は `gated` を canonical term として使う
- **AND** blocker の概要が添えられる

#### Scenario: acceptance uses fail for repository-fixable issues
- **GIVEN** acceptance が code / tests / tasks / spec の repository 内修正で解決できる問題を見つけた
- **WHEN** reviewer が machine-readable verdict を返す
- **THEN** verdict は `fail` を使う
- **AND** findings は apply に戻って自律修正可能な内容だけを列挙する

#### Scenario: rejecting review requests gated hold instead of immediate apply resume
- **GIVEN** rejecting review が apply-generated rejection proposal を評価している
- **AND** reviewer は change 全体の reject には同意しない
- **AND** 追加情報、仕様整理、fixture 再設計、または依存解消がないと apply を再開しても同じ blocker が再発すると判断する
- **WHEN** reviewer が machine-readable verdict を返す
- **THEN** verdict は immediate apply resume とは区別された gated hold outcome になる
- **AND** runtime は change を acceptance-gated / blocked hold 文脈で扱う前提を保つ
