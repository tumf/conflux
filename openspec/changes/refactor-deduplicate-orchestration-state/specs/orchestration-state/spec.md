## MODIFIED Requirements

### Requirement: OrchestratorState が唯一のループ状態ソースである
`OrchestratorState` はオーケストレーションループの状態（apply 回数、pending/archived/completed 変更セット、イテレーション番号、current change ID、max_iterations）の唯一の正規ソースでなければならない（MUST）。

`Orchestrator` struct および `tui::orchestrator::run_orchestrator` 関数は、これらの値をローカルフィールド/変数として独自に保持してはならない（SHALL NOT）。ただし `max_iterations` のように `OrchestratorState` 初期化パラメータとして一度だけ使う値はコンストラクタ引数として保持してよい（MAY）。

#### Scenario: ループ中の状態参照は shared_state 経由
- **WHEN** オーケストレーションループ内で iteration 数や changes_processed を参照する
- **THEN** `shared_state.read().await.iteration()` や `shared_state.read().await.changes_processed()` 等の `OrchestratorState` メソッドが使用される
- **AND** ローカル変数やフィールドからは参照されない
