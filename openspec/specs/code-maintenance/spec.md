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

### Requirement: Execution Module Foundation

システムは `src/execution/` モジュールを提供し、serial mode と parallel mode で共通して使用可能な実行コンテキストと結果型を定義しなければならない（SHALL）。

#### Scenario: ExecutionContext の作成

- **GIVEN** 変更 ID とコンフィグが利用可能である
- **WHEN** 実行コンテキストを作成する
- **THEN** `ExecutionContext` 構造体が作成される
- **AND** workspace_path は serial mode では None、parallel mode では Some(path)

#### Scenario: ExecutionResult の状態遷移

- **GIVEN** 実行処理が開始された
- **WHEN** 処理が完了する
- **THEN** `ExecutionResult::Success`, `ExecutionResult::Failed`, または `ExecutionResult::Cancelled` のいずれかが返される

### Requirement: Progress Information Tracking

システムは実行の進捗情報（完了タスク数、総タスク数、完了率）を追跡するための共通型を提供しなければならない（SHALL）。

#### Scenario: ProgressInfo の計算

- **GIVEN** completed = 3, total = 10 の進捗情報がある
- **WHEN** 完了率を計算する
- **THEN** 30% が返される

#### Scenario: ゼロ除算の回避

- **GIVEN** completed = 0, total = 0 の進捗情報がある
- **WHEN** 完了率を計算する
- **THEN** 0% が返される（ゼロ除算エラーなし）


### Requirement: Common Apply Iteration Logic

システムは、apply コマンドの反復実行を管理するための共通ロジックを提供しなければならない（SHALL）。このロジックは serial mode と parallel mode の両方で使用される。

#### Scenario: 単一 apply の実行

- **GIVEN** change_id = "my-change" と apply コマンドが設定されている
- **WHEN** `execute_apply_iteration()` を呼び出す
- **THEN** apply コマンドが実行される
- **AND** 実行後の進捗情報が返される

#### Scenario: 反復 apply の実行

- **GIVEN** max_iterations = 50 が設定されている
- **WHEN** タスクが 100% 完了するまで反復する
- **THEN** 各反復で進捗をチェックする
- **AND** 完了したら反復を終了する

#### Scenario: 最大反復回数の制限

- **GIVEN** max_iterations = 50 が設定されている
- **WHEN** 50 回の反復後もタスクが完了しない
- **THEN** エラーが返される

### Requirement: Common Progress Commit Creation

システムは、進捗コミットを作成するための共通関数を提供しなければならない（SHALL）。この関数は VCS の種類に関係なく動作する。

#### Scenario: jj でのプログレスコミット

- **GIVEN** VCS backend が jj である
- **WHEN** `create_progress_commit()` を呼び出す
- **THEN** `jj describe` でコミットメッセージが設定される

#### Scenario: git でのプログレスコミット

- **GIVEN** VCS backend が git である
- **WHEN** `create_progress_commit()` を呼び出す
- **THEN** `git commit` でコミットが作成される

### Requirement: VCS Operations through WorkspaceManager

parallel/executor.rs 内の VCS 操作は、直接コマンドを実行する代わりに `WorkspaceManager` trait を使用しなければならない（SHALL）。

#### Scenario: コミットメッセージの設定

- **GIVEN** workspace_path でコミットメッセージを設定する必要がある
- **WHEN** `workspace_manager.set_commit_message()` を呼び出す
- **THEN** VCS backend に応じた適切なコマンドが実行される

#### Scenario: リビジョンの取得

- **GIVEN** workspace の現在のリビジョンを取得する必要がある
- **WHEN** `workspace_manager.get_revision_in_workspace()` を呼び出す
- **THEN** 現在のリビジョン ID が返される

### Requirement: Common Archive Verification

システムは、アーカイブ操作の成功を検証するための共通関数を提供しなければならない（SHALL）。この関数は serial mode と parallel mode の両方で使用される。

#### Scenario: アーカイブ成功の検証

- **GIVEN** change_id = "my-change" のアーカイブ操作が完了した
- **WHEN** `verify_archive_completion()` を呼び出す
- **THEN** change が `openspec/changes/` から削除されていることを確認する
- **AND** change が `openspec/changes/archive/` に存在することを確認する

#### Scenario: 日付プレフィックス付きアーカイブの検証

- **GIVEN** change_id = "my-change" がアーカイブされた
- **WHEN** アーカイブディレクトリ名が "2026-01-12-my-change" 形式である
- **THEN** `verify_archive_completion()` は成功を返す

#### Scenario: アーカイブ失敗の検出

- **GIVEN** archive コマンドが実行されたが、ファイルが移動されていない
- **WHEN** `verify_archive_completion()` を呼び出す
- **THEN** エラーが返され、change ディレクトリがまだ存在することを示す

### Requirement: Common Task Completion Verification

システムは、タスクの完了状態を検証するための共通関数を提供しなければならない（SHALL）。

#### Scenario: タスク完了の確認

- **GIVEN** tasks.md に 10 個のタスクがあり、10 個が完了している
- **WHEN** `verify_task_completion()` を呼び出す
- **THEN** true が返される

#### Scenario: タスク未完了の確認

- **GIVEN** tasks.md に 10 個のタスクがあり、7 個が完了している
- **WHEN** `verify_task_completion()` を呼び出す
- **THEN** false が返され、進捗情報 (7/10) が含まれる

### Requirement: Common Archive Command Execution

システムは、archive コマンドを実行するための共通関数を提供しなければならない（SHALL）。この関数は workspace_path を受け取り、指定された場所でコマンドを実行する。

#### Scenario: メインワークスペースでの実行

- **GIVEN** workspace_path = None
- **WHEN** `execute_archive_command()` を呼び出す
- **THEN** カレントディレクトリで archive コマンドが実行される

#### Scenario: 別ワークスペースでの実行

- **GIVEN** workspace_path = Some("/path/to/workspace")
- **WHEN** `execute_archive_command()` を呼び出す
- **THEN** 指定されたワークスペースで archive コマンドが実行される
