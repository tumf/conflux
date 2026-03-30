## MODIFIED Requirements

### Requirement: OrchestratorState が唯一のループ状態ソースである
`OrchestratorState` はオーケストレーションループの状態（apply 回数、pending/archived/completed 変更セット、イテレーション番号、current change ID）の唯一の正規ソースでなければならない（MUST）。

`Orchestrator` struct および `tui::orchestrator::run_orchestrator` 関数は、これらのカウンタやセットをローカルフィールド/変数として独自に保持してはならない（SHALL NOT）。

状態の参照は `shared_state.read().await` 経由で行い、状態の変更は `apply_execution_event()` または `apply_command()` 経由で行わなければならない（MUST）。

#### Scenario: Orchestrator struct がローカル apply_counts を持たない
- **WHEN** `Orchestrator` struct の定義を確認する
- **THEN** `apply_counts`, `changes_processed`, `iteration`, `current_change_id` フィールドが存在しない
- **AND** これらの値は `self.shared_state.read().await` 経由で取得される

#### Scenario: TUI orchestrator がローカル pending_changes を持たない
- **WHEN** `tui::orchestrator::run_orchestrator` 関数の実装を確認する
- **THEN** `apply_counts`, `pending_changes`, `changes_processed`, `total_changes` のローカル変数が存在しない
- **AND** これらの値は `shared_state.read().await` 経由で取得される

#### Scenario: ステート一貫性の保証
- **WHEN** serial モードでの実行中に Change が archived される
- **THEN** `OrchestratorState` の `pending_changes` が減少する
- **AND** `changes_processed` が増加する
- **AND** 他に同じ情報を保持する変数が更新される必要がない
