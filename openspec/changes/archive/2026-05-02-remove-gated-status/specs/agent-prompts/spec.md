## MODIFIED Requirements

### Requirement: Acceptance prompt MUST evaluate implementation blockers

acceptance プロンプトは Implementation Blocker を審査し、妥当と判断した場合は compatibility verdict として `ACCEPTANCE: GATED` / `{"acceptance":"gated"}` を出力してもよい（MAY）。ただし、この verdict は user-facing lifecycle/status としての `gated` を意味してはならず、runtime は paused state を `stalled` として扱わなければならない（MUST）。

acceptance は以下を満たさなければならない（MUST）。
- `Implementation Blocker` の内容が不十分または誤りの場合は `ACCEPTANCE: FAIL` を出力し、follow-up タスクを tasks.md に追加する
- acceptance blocker verdict の場合は blocker の概要を簡潔に出力する
- repo 内編集・テスト更新・spec/tasks/proposal 修正など、AI がこの repository 内の変更だけで自律的に解決できる問題は `FAIL` として返さなければならない（MUST）。
- 人判断待ち、repo 外の設定変更、外部依存の解消待ち、追加情報待ち、または apply を再実行しても repository 変更だけでは解決不能な blocker は acceptance blocker verdict として返さなければならない（MUST）。
- blocker verdict を返さず `FAIL` を返した場合、その finding は apply へ戻って repository 作業を行うことで解消可能であることを意味しなければならない（MUST）。
- apply-generated recoverable blocker を審査するレビュー経路では、「change を reject するか」と「change を stalled hold のまま保留するか」を区別できなければならない
- 互換期間中に旧 `blocked` acceptance verdict や `gated` verdict を runtime が受理できても、新規 lifecycle/status contract は `stalled` を operator-facing term として使わなければならない（MUST）

#### Scenario: acceptance emits blocker verdict for a valid implementation blocker
- **GIVEN** acceptance が妥当な Implementation Blocker を確認した
- **AND** その blocker は repository 内編集だけでは解決できない
- **WHEN** reviewer が machine-readable verdict を返す
- **THEN** verdict は acceptance blocker として解釈される
- **AND** runtime/user-facing status は `stalled` として扱われる
- **AND** blocker の概要が添えられる

#### Scenario: acceptance uses fail for repository-fixable issues
- **GIVEN** acceptance が code / tests / tasks / spec の repository 内修正で解決できる問題を見つけた
- **WHEN** reviewer が machine-readable verdict を返す
- **THEN** verdict は `fail` を使う
- **AND** findings は apply に戻って自律修正可能な内容だけを列挙する

#### Scenario: rejecting review requests stalled hold instead of immediate apply resume
- **GIVEN** rejecting review が apply-generated rejection proposal を評価している
- **AND** reviewer は change 全体の reject には同意しない
- **AND** 追加情報、仕様整理、fixture 再設計、または依存解消がないと apply を再開しても同じ blocker が再発すると判断する
- **WHEN** reviewer が machine-readable verdict を返す
- **THEN** verdict は immediate apply resume とは区別された stalled hold outcome になる
- **AND** runtime は change を stalled hold 文脈で扱う前提を保つ
