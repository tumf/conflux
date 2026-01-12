# code-maintenance Specification

## Purpose
Defines code maintenance guidelines and codebase health requirements.
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

### Requirement: VCS Abstraction Layer

システムは VCS バックエンド（Git, Jujutsu）を統一されたトレイトベースの抽象化で管理しなければならない (SHALL)。

各 VCS 実装は専用サブモジュール (`src/vcs/jj/`, `src/vcs/git/`) に配置しなければならない (MUST)。
共通ロジックは `src/vcs/commands.rs` に集約すること。

#### Scenario: 新しい VCS バックエンドを追加する場合

- **WHEN** 開発者が新しい VCS バックエンドを追加する
- **THEN** `src/vcs/<backend>/` にモジュールを作成し、`WorkspaceManager` トレイトを実装するだけで統合可能

#### Scenario: VCS コマンド実行エラー

- **WHEN** VCS コマンドが失敗する
- **THEN** システムは `VcsError` 型で統一されたエラーを返す
- **AND** エラーにはバックエンド種別と詳細メッセージが含まれる

### Requirement: Parallel Execution Module Structure

並列実行機能は `src/parallel/` モジュール配下に責務ごとに分離されたサブモジュールとして構成されなければならない (SHALL)。

各サブモジュールは単一責務の原則に従い、個別にテスト可能でなければならない (MUST)。

#### Scenario: モジュール構成

- **WHEN** 開発者が並列実行機能を調査する
- **THEN** 以下のモジュール構成が確認できる
  - `parallel/mod.rs` - オーケストレーション
  - `parallel/types.rs` - 共通型
  - `parallel/events.rs` - イベント定義
  - `parallel/cleanup.rs` - クリーンアップ処理
  - `parallel/conflict.rs` - コンフリクト処理
  - `parallel/executor.rs` - 実行ロジック

#### Scenario: 個別モジュールのテスト

- **WHEN** 開発者がコンフリクト処理のみを変更する
- **THEN** `parallel/conflict.rs` のテストのみを実行して検証可能

### Requirement: Unified Orchestration Module

The codebase SHALL have a unified orchestration module that contains shared logic between CLI and TUI modes.

#### Scenario: Archive logic is shared
- **WHEN** a change is archived in CLI mode
- **AND** when a change is archived in TUI mode
- **THEN** both modes SHALL use the same `orchestration::archive_change()` function
- **AND** the archive path validation SHALL use `openspec/changes/archive/`

#### Scenario: Apply logic is shared
- **WHEN** a change is applied in CLI mode
- **AND** when a change is applied in TUI mode
- **THEN** both modes SHALL use the same `orchestration::apply_change()` function
- **AND** hook invocations (pre_apply, post_apply, on_error) SHALL be consistent

#### Scenario: State management is shared
- **WHEN** orchestration state is tracked in CLI mode
- **AND** when orchestration state is tracked in TUI mode
- **THEN** both modes SHALL use the same `OrchestratorState` structure
- **AND** variable naming SHALL be consistent (pending_changes, completed_changes, apply_counts)

### Requirement: OutputHandler Abstraction

The orchestration module SHALL provide an OutputHandler trait for mode-specific output handling.

#### Scenario: CLI uses logging output
- **WHEN** CLI mode executes orchestration
- **THEN** output SHALL be written to the tracing log
- **AND** no channel communication is required

#### Scenario: TUI uses channel output
- **WHEN** TUI mode executes orchestration
- **THEN** output SHALL be sent through mpsc channels
- **AND** output SHALL be displayed in the TUI log panel

### Requirement: Hook Context Helpers

The orchestration module SHALL provide helper functions for building HookContext instances.

#### Scenario: Archive hook context
- **WHEN** archive operation needs hook context
- **THEN** `build_archive_context()` helper SHALL be used
- **AND** the helper SHALL set all required fields consistently

#### Scenario: Apply hook context
- **WHEN** apply operation needs hook context
- **THEN** `build_apply_context()` helper SHALL be used
- **AND** the helper SHALL set all required fields consistently

