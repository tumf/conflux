## MODIFIED Requirements

### Requirement: TUI ステータス表示は Reducer から導出される
TUI の Change ステータス表示（文字列・色）は `ChangeRuntimeState::display_status()` および `display_color()` から導出されなければならない（MUST）。

TUI 固有のステータス enum（旧 `QueueStatus`）を保持してはならない（SHALL NOT）。`ChangeState` は表示用の文字列キャッシュ（`display_status_cache`）と色キャッシュ（`display_color_cache`）のみを持ち、これらは Reducer のスナップショットから更新される。

#### Scenario: TUI が Reducer からステータスを読み取る
- **WHEN** TUI が Change のステータスを表示する
- **THEN** `ChangeState.display_status_cache` の文字列が使用される
- **AND** `ChangeState.display_color_cache` の色が使用される
- **AND** `QueueStatus` enum が codebase に存在しない

#### Scenario: イベント受信時のキャッシュ更新
- **WHEN** `OrchestratorEvent::ProcessingStarted` を TUI が受信する
- **THEN** `ChangeState.display_status_cache` が `"applying"` に更新される
- **AND** `ChangeState.display_color_cache` が `Color::Cyan` に更新される
