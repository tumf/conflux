## MODIFIED Requirements

### Requirement: TUI ステータス表示は Reducer から導出される

TUI の Change ステータス表示（文字列・色）は `ChangeRuntimeState::display_status()` および `display_color()` から導出されなければならない（MUST）。

TUI 固有のステータス enum（旧 `QueueStatus`）を保持してはならない（SHALL NOT）。`ChangeState` は表示用の文字列キャッシュ（`display_status_cache`）と色キャッシュ（`display_color_cache`）のみを持ち、これらは Reducer のスナップショットまたは同じ workspace/git-derived refresh evidence から更新される。

TUI はログ表示の重複抑制のために transient な観測用状態を保持してよい（MAY）が、その状態を reducer-derived display status、scheduler dispatch、resume routing、acceptance、archive、または next-action decision の入力として使ってはならない（MUST NOT）。

Refresh-time `merge_wait_ids` は、archived-but-not-merged workspace evidence から導出された TUI display synchronization input として扱ってよい（MAY）。TUI はこれを scheduler dispatch、resume routing、acceptance、archive、または next-action decision の入力として使ってはならない（MUST NOT）。

#### Scenario: TUI が Reducer からステータスを読み取る

- **WHEN** TUI が Change のステータスを表示する
- **THEN** `ChangeState.display_status_cache` の文字列が使用される
- **AND** `ChangeState.display_color_cache` の色が使用される
- **AND** `QueueStatus` enum が codebase に存在しない

#### Scenario: イベント受信時のキャッシュ更新

- **WHEN** `OrchestratorEvent::ProcessingStarted` を TUI が受信する
- **THEN** `ChangeState.display_status_cache` が `"applying"` に更新される
- **AND** `ChangeState.display_color_cache` が `Color::Cyan` に更新される

#### Scenario: refresh-derived merge wait corrects stale resolve pending display

- **GIVEN** change `alpha` is displayed as `resolve pending`
- **AND** the refresh loop observes `alpha` as archive-complete but not merged into base
- **WHEN** the TUI handles `OrchestratorEvent::ChangesRefreshed` with `alpha` in `merge_wait_ids`
- **THEN** `alpha` is displayed as `merge wait`
- **AND** the display correction does not enqueue, dispatch, archive, accept, or otherwise route workflow execution

#### Scenario: stale merge wait refresh does not regress terminal display

- **GIVEN** change `alpha` is already displayed as `merged` or `rejected`
- **WHEN** the TUI handles a stale `OrchestratorEvent::ChangesRefreshed` that includes `alpha` in `merge_wait_ids`
- **THEN** `alpha` remains displayed as its terminal state
- **AND** the terminal row is not regressed to `merge wait`
