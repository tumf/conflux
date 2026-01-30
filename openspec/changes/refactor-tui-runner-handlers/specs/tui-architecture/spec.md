## MODIFIED Requirements
### Requirement: No Behavioral Changes

TUI のリファクタリングは、実行時の挙動を変更してはならない（SHALL NOT）。

`run_tui_loop` のキー入力処理および TuiCommand 処理は、可読性向上のためにヘルパー関数へ分割してもよい（MAY）。ただし、既存のショートカット・表示・状態遷移の挙動は維持しなければならない（MUST）。

#### Scenario: All existing tests pass

- **WHEN** `cargo test` is run after refactoring
- **THEN** all tests that passed before refactoring still pass
- **AND** no new test failures are introduced

#### Scenario: TUI functionality unchanged

- **GIVEN** the TUI is started with `cargo run -- tui`
- **WHEN** user interacts with the TUI
- **THEN** all keyboard shortcuts work as before
- **AND** all display elements render identically
- **AND** all state transitions behave identically
