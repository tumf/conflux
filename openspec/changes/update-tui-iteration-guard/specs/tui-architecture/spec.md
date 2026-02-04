## MODIFIED Requirements
### Requirement: TUI Module Structure

TUI モジュールは `src/tui/` 配下のディレクトリ構成で整理され、TUI state 層は共有オーケストレーション状態から change の進捗と実行メタデータを取得しなければならない（SHALL）。UI 固有の状態（カーソル、ビュー、選択状態など）は TUI 側で保持する。
共有状態から取り込む iteration は、既に表示されている値より小さい場合に上書きしてはならない。表示された iteration が後退しないよう、より大きい値を維持しなければならない。
さらに、出力イベントにより iteration を更新する際は、現在の `queue_status` に一致するステージのイベントのみを反映し、同一ステージ内で iteration が単調増加となるように更新しなければならない。ステージ開始時は iteration 表示をリセットし、前ステージの値を持ち越してはならない。この更新規則は MUST とする。

#### Scenario: Module directory exists
- **WHEN** the project is compiled
- **THEN** `src/tui/mod.rs` exists as the module entry point
- **AND** submodules are organized in `src/tui/*.rs` files

#### Scenario: Each submodule has single responsibility
- **GIVEN** the TUI module structure
- **THEN** `types.rs` contains only enum and type definitions
- **AND** `state.rs` contains only state management logic
- **AND** `events.rs` contains only event and command definitions
- **AND** `render.rs` contains only rendering functions
- **AND** `orchestrator.rs` contains only orchestration logic
- **AND** `runner.rs` contains only the main TUI loop
- **AND** `queue.rs` contains only DynamicQueue implementation
- **AND** `utils.rs` contains only utility functions
- **AND** `terminal.rs` contains only terminal execution helpers
- **AND** `worktrees.rs` contains only worktree-related helpers

#### Scenario: Change progress uses shared state
- **GIVEN** the TUI state layer builds the change list for rendering
- **WHEN** change progress and execution metadata are required
- **THEN** the data source is the shared orchestration state
- **AND** UI-specific fields remain in TUI-owned state

#### Scenario: Iteration number does not regress during refresh
- **GIVEN** the TUI already displays `iteration_number=4` for a change
- **AND** the shared orchestration state reports `apply_count=3`
- **WHEN** the automatic refresh merges shared state into the TUI change list
- **THEN** the TUI keeps `iteration_number=4`

#### Scenario: Output events do not regress iteration within a stage
- **GIVEN** the TUI displays `queue_status=Archiving` and `iteration_number=2`
- **WHEN** an older `ArchiveOutput` event with `iteration=1` arrives
- **THEN** the TUI keeps `iteration_number=2`

#### Scenario: Output events from other stages do not overwrite iteration
- **GIVEN** the TUI displays `queue_status=Resolving` and `iteration_number=2`
- **WHEN** an `ApplyOutput` event arrives for the same change
- **THEN** the TUI keeps `iteration_number=2`

#### Scenario: Stage transition resets iteration display
- **GIVEN** the TUI displays `queue_status=Applying` and `iteration_number=3`
- **WHEN** `ArchiveStarted` is handled for the same change
- **THEN** the TUI clears the iteration display for the new stage
