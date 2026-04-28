## ADDED Requirements

### Requirement: AgentRunner の現役実行経路を単一化する

システムは Agent 実行の正系フローとして、AiCommandRunner ベースの現役経路を明確に維持しなければならない（SHALL）。

レガシー entrypoint を互換上の理由で残す場合でも、それらは現役フローと混在しない明示的な境界に隔離され、prompt 展開順・履歴注入順・出力伝播の公開挙動を変えてはならない（MUST）。

#### Scenario: apply / acceptance / archive / analyze / resolve の正系挙動が維持される

- **GIVEN** CLI / TUI / server が Agent 実行を開始する
- **WHEN** apply / acceptance / archive / analyze / resolve のいずれかを実行する
- **THEN** prompt 展開順、履歴注入順、出力伝播はリファクタ前と同じである
- **AND** 利用者から見える API / CLI の挙動は変化しない

#### Scenario: レガシー entrypoint は現役フローから分離される

- **GIVEN** 開発者が `src/agent/runner.rs` 周辺の実装を調査する
- **WHEN** AgentRunner の実行 entrypoint を確認する
- **THEN** 現役経路とレガシー経路の境界が明確である
- **AND** 不要な `#[allow(dead_code)]` が正系フローの理解を妨げない
