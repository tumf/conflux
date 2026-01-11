## ADDED Requirements

### Requirement: Config Module Structure

設定管理機能は `src/config/` モジュール配下に責務ごとに分離されたサブモジュールとして構成されなければならない (SHALL)。

JSONC パーサーは汎用モジュールとして他からも利用可能でなければならない (MUST)。

#### Scenario: モジュール構成

- **WHEN** 開発者が設定管理を調査する
- **THEN** 以下のモジュール構成が確認できる
  - `config/mod.rs` - OrchestratorConfig 本体
  - `config/defaults.rs` - デフォルト値
  - `config/expand.rs` - プレースホルダー展開
  - `config/jsonc.rs` - JSONC パーサー

#### Scenario: JSONC パーサーの再利用

- **WHEN** 他のモジュールが JSONC ファイルをパースする必要がある
- **THEN** `config::jsonc::parse()` を呼び出して利用可能
