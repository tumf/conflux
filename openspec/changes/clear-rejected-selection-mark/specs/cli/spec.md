## MODIFIED Requirements

### Requirement: Archived 状態の checkbox 表示

TUI は terminal row の checkbox / execution mark semantics を、その row が execution candidate かどうかに応じて表現しなければならない（SHALL）。

Archived 状態の change は既存どおり checkbox をグレー表示してよい。一方で rejected 状態の change は execution candidate ではないため、以前の execution mark を保持したまま表示してはならない（MUST NOT）。

#### Scenario: rejected 状態では x マークを保持しない

- **GIVEN** TUI が change 一覧を表示している
- **AND** ある change が rejection flow 完了により `rejected` 状態へ遷移した
- **WHEN** 画面が次にレンダリングされる
- **THEN** その change は execution mark なし (`selected = false`) で表示される
- **AND** ステータス表示は `rejected` のままである
