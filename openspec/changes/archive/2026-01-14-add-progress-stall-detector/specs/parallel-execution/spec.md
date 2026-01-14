# parallel-execution Specification

## MODIFIED Requirements

### Requirement: 停止 change の依存スキップと継続実行

parallel 実行において、サーキットブレーカーにより change が停止（failed 扱い）となった場合、その change に依存する change をスキップし、依存しない change の実行を継続しなければならない（SHALL）。

#### Scenario: 依存 change はスキップされ、独立 change は継続される
- **GIVEN** change `A` が stall により停止（failed 扱い）となった
- **AND** change `C` は `A` に依存している
- **AND** change `B` は `A` に依存していない
- **WHEN** parallel executor が次の実行対象を決定する
- **THEN** `C` は依存失敗としてスキップされる
- **AND** `B` は実行を継続できる
