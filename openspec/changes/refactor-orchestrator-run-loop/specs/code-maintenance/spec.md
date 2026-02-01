## MODIFIED Requirements
### Requirement: Unified Orchestration Module
The codebase SHALL have a unified orchestration module that contains shared logic between CLI and TUI modes, including a SerialRunService that owns the shared serial execution flow.

オーケストレーションの entry ループ（`run` など）は、初期化、停止/キャンセル判定、change の更新・選定、結果処理の責務をヘルパー関数へ分割してもよい（MAY）。ただし、serial/parallel の共有フローと挙動は維持しなければならない（MUST）。

#### Scenario: Serial run is routed through a shared service
- **WHEN** the orchestrator runs in CLI serial mode
- **AND** when the orchestrator runs in TUI serial mode
- **THEN** both modes SHALL invoke SerialRunService for the shared serial execution flow
- **AND** mode-specific output and UI updates are handled by injected adapters
