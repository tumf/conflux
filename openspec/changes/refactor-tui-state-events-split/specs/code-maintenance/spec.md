## MODIFIED Requirements
### Requirement: TUI State Module Structure
TUI の状態管理機能は `src/tui/state/` モジュール配下に責務ごとに分離されたサブモジュールとして構成されなければならない (SHALL)。
`AppState` 構造体自体は変更せず、内部メソッドの実装を適切なモジュールに分散しなければならない (MUST)。

#### Scenario: モジュール構成
- **WHEN** 開発者が TUI 状態管理を調査する
- **THEN** 以下のモジュール構成が確認できる
  - `state/mod.rs` - AppState 本体
  - `state/change.rs` - ChangeState
  - `state/modes.rs` - モード管理
  - `state/logs.rs` - ログ管理
  - `state/events/mod.rs` - イベント処理入口
  - `state/events/processing.rs` - 実行開始系イベント
  - `state/events/completion.rs` - 完了系イベント
  - `state/events/progress.rs` - 進捗更新イベント
  - `state/events/refresh.rs` - リフレッシュイベント

#### Scenario: ログ機能の変更
- **WHEN** 開発者がログ表示機能を変更する
- **THEN** `state/logs.rs` のみを変更すればよい
- **AND** 他のモジュールへの影響は最小限
