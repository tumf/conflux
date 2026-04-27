## MODIFIED Requirements

### Requirement: Dashboard UI - Change List Display

Webダッシュボードは、TUI の表示語彙と一致するステータス語彙で change 一覧を表示しなければならない（SHALL）。

Rejected row は read-only terminal row として表示してよいが、execution mark を保持した active candidate として表現してはならない（MUST NOT）。

#### Scenario: rejected row is displayed without execution mark

- **GIVEN** Web UI が change 一覧を表示している
- **AND** ある change の status が `rejected` である
- **WHEN** dashboard row がレンダリングされる
- **THEN** その row は `rejected` の visual treatment で表示される
- **AND** row は `selected = false` として扱われる
- **AND** active execution candidate と同じ checkbox semantics を保持しない
