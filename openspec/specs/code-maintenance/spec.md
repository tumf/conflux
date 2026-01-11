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

### Requirement: TUI State Module Structure

TUI の状態管理機能は `src/tui/state/` モジュール配下に責務ごとに分離されたサブモジュールとして構成されなければならない (SHALL)。

`AppState` 構造体自体は変更せず、内部メソッドの実装を適切なモジュールに分散しなければならない (MUST)。

#### Scenario: モジュール構成

- **WHEN** 開発者が TUI 状態管理を調査する
- **THEN** 以下のモジュール構成が確認できる
  - `state/mod.rs` - AppState 本体
  - `state/change.rs` - ChangeState
  - `state/modes.rs` - モード管理
  - `state/logs.rs` - ログ管理
  - `state/events.rs` - イベント処理

#### Scenario: ログ機能の変更

- **WHEN** 開発者がログ表示機能を変更する
- **THEN** `state/logs.rs` のみを変更すればよい
- **AND** 他のモジュールへの影響は最小限

