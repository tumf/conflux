## MODIFIED Requirements

### Requirement: Public API Stability

TUI モジュールは公開エクスポートを維持しなければならない（SHALL）。

ただし、本プロジェクト内の機能追加に伴い、`OrchestratorEvent` および `TuiCommand` への **バリアント追加**は許容される（MAY）。

既存バリアントの意味・フィールド・名称の互換性は維持されなければならない（MUST）。

#### Scenario: 既存バリアントを壊さずに追加できる
- **GIVEN** external code imports from the tui module
- **WHEN** new variants are added to `OrchestratorEvent` or `TuiCommand`
- **THEN** existing variants remain available and unchanged
- **AND** the module continues to compile and run within this repository
