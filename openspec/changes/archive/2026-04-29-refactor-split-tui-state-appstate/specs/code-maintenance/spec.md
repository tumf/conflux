## MODIFIED Requirements

### Requirement: TUI状態ロジックの責務分離後も公開挙動を維持する

システムは TUI 状態管理の内部構造を整理しても、利用者から見える選択・キュー・resume / retry・ワークツリー・ログ関連の挙動を変更してはならない。

`src/tui/state.rs` は入口・型定義・委譲中心に保ってよく（MAY）、AppState の主要ロジックは責務別サブモジュールへ移してよい（MAY）。ただし reducer 同期、display status、TuiCommand 生成の意味論は既存どおりでなければならない（MUST）。

#### Scenario: resume / retry と queue 同期が維持される

- **GIVEN** 利用者が TUI で change を queue し、Stopped または Error 状態から resume / retry を行う
- **WHEN** 状態更新が実行される
- **THEN** 選択状態、queue 意図、shared reducer 同期、TuiCommand の結果は既存どおりに維持される

#### Scenario: ログと worktree の操作挙動が維持される

- **GIVEN** 利用者がログスクロール、ログ panel toggle、または worktree カーソル操作を行う
- **WHEN** AppState 更新が実行される
- **THEN** 表示オフセット、auto-scroll、cursor 移動、guard 判定は既存どおりである
- **AND** `src/tui/state.rs` の構造整理は利用者可視の挙動を変えない
