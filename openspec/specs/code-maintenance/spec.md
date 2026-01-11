# code-maintenance Specification

## Purpose
TBD - created by archiving change refactor-codebase-cleanup. Update Purpose after archive.
## Requirements
### Requirement: コマンド実行ロジックの共通化
オーケストレーターは `jj`/シェル実行に関する重複ロジックを共通ヘルパーへ集約し、既存の出力・エラー扱いを維持するために SHALL 共通ヘルパーを使用しなければならない。

#### Scenario: jj 実行の失敗時に既存同等のエラーを返す
- **WHEN** 共通ヘルパーで `jj` コマンドが非0終了する
- **THEN** 既存と同等のエラーメッセージが返される

### Requirement: レガシー／未使用コードの整理
オーケストレーターは未使用のレガシーモジュールや `#[allow(dead_code)]` で保護された不要コードを削除または明示的に隔離するために MUST 整理方針を適用しなければならない。

#### Scenario: 未使用コードを整理した後でもビルドが成功する
- **WHEN** 未使用コードの整理後にビルドを実行する
- **THEN** `cargo build` が成功する

### Requirement: リファクタリング安全性の担保
オーケストレーターはリファクタリング後も既存仕様の挙動を保ち、検証手順で後退がないことを示すために SHALL 検証を通過しなければならない。

#### Scenario: 既存の検証が通過する
- **WHEN** `cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` を実行する
- **THEN** すべて成功する

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

