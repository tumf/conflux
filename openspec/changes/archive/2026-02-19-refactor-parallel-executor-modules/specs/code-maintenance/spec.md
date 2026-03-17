## ADDED Requirements
### Requirement: Parallel Executor Implementation Split
並列実行モジュールは `parallel/mod.rs` を入口と再公開に集中させ、`ParallelExecutor` の詳細実装は責務別サブモジュールに配置しなければならない (SHALL)。

#### Scenario: 入口モジュールの簡素化
- **WHEN** 開発者が `src/parallel/` の構成を確認する
- **THEN** `parallel/mod.rs` はモジュール宣言と再公開が中心である
- **AND** `ParallelExecutor` の詳細実装は別のサブモジュールに存在する
