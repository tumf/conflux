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
  - `state/events/mod.rs` - イベント処理入口
  - `state/events/processing.rs` - 実行開始系イベント
  - `state/events/completion.rs` - 完了系イベント
  - `state/events/progress.rs` - 進捗更新イベント
  - `state/events/refresh.rs` - リフレッシュイベント

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

#### Scenario: Git コマンドの責務分割
- **WHEN** 開発者が Git コマンド実装を調査する
- **THEN** commands モジュールが責務別（basic/commit/worktree/merge）に分割されている

### Requirement: Parallel Execution Module Structure
並列実行機能は `src/parallel/` モジュール配下に責務ごとに分離されたサブモジュールとして構成されなければならない (SHALL)。
各サブモジュールは単一責務の原則に従い、個別にテスト可能でなければならない (MUST)。

#### Scenario: モジュール構成
- **WHEN** 開発者が並列実行機能を調査する
- **THEN** 以下のモジュール構成が確認できる
  - `parallel/mod.rs` - 入口と再公開
  - `parallel/types.rs` - 共通型
  - `parallel/events.rs` - イベント定義
  - `parallel/cleanup.rs` - クリーンアップ処理
  - `parallel/conflict.rs` - コンフリクト処理
  - `parallel/executor.rs` - 実行ロジック
  - `parallel/workspace.rs` - ワークスペース管理
  - `parallel/dynamic_queue.rs` - 動的キュー管理
  - `parallel/merge.rs` - マージと解決処理

#### Scenario: 個別モジュールのテスト
- **WHEN** 開発者がコンフリクト処理のみを変更する
- **THEN** `parallel/conflict.rs` のテストのみを実行して検証可能

### Requirement: Unified Orchestration Module
The codebase SHALL have a unified orchestration module that contains shared logic between CLI and TUI modes, including a SerialRunService that owns the shared serial execution flow.

#### Scenario: Serial run is routed through a shared service
- **WHEN** the orchestrator runs in CLI serial mode
- **AND** when the orchestrator runs in TUI serial mode
- **THEN** both modes SHALL invoke SerialRunService for the shared serial execution flow
- **AND** mode-specific output and UI updates are handled by injected adapters

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

archive 検証の失敗理由は、次回の archive 試行のプロンプトに含められるように構造化されなければならない（SHALL）。

#### Scenario: 検証失敗理由の構造化

- **GIVEN** archive 検証が失敗した
- **WHEN** 検証結果が返される
- **THEN** 失敗理由には具体的な情報が含まれる
- **AND** 理由は履歴コンテキストに含められる形式である
- **AND** 例: "Change still exists at openspec/changes/{change_id}"

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

`archive_change()` および `archive_change_streaming()` は、archive コマンドの実行結果を記録し、再試行時には前回の履歴をプロンプトに含めなければならない（MUST）。

archive ループの実装は、フック実行・コマンド実行・検証・履歴記録をヘルパー関数に分割してもよい（MAY）。ただし、履歴の記録と再試行時の伝播は必ず維持しなければならない（MUST）。

#### Scenario: Archive 実行後の履歴記録

- **GIVEN** システムが change の archive を実行する
- **WHEN** archive コマンドが完了する（成功または失敗）
- **THEN** システムは試行結果を記録する
- **AND** 記録には試行回数、成功/失敗ステータス、所要時間、検証結果が含まれる

#### Scenario: Archive 再試行時の履歴伝播

- **GIVEN** 1回目の archive が検証失敗した
- **WHEN** システムが同じ change の archive を再試行する
- **THEN** `AgentRunner::run_archive_streaming()` に渡されるプロンプトに前回の履歴が含まれる
- **AND** 履歴には検証失敗の理由（"Change still exists at...") が含まれる

#### Scenario: Archive 成功時の履歴クリア

- **GIVEN** change の archive が成功した
- **WHEN** change が完全に処理される
- **THEN** その change の archive 履歴はクリアされる

### Requirement: Serial/Parallel 実行フローの共有化
システムは serial/parallel モードで共通となる apply・archive・進捗更新の処理を共有関数に集約しなければならない（SHALL）。

#### Scenario: serial/parallel が同じ共有関数を利用する
- **WHEN** serial モードで change を apply する
- **THEN** apply/archiving/進捗更新は共通関数経由で実行される
- **AND** parallel モードでも同じ共通関数が使用される

#### Scenario: モード固有の差分が分離される
- **WHEN** モード固有の出力やイベント送信を実装する
- **THEN** 共有関数は純粋な実行フローのみを扱う
- **AND** 出力/イベントの責務は呼び出し側に分離される

### Requirement: Agent モジュールの責務分割
オーケストレーターは Agent の実行・出力処理・履歴管理・プロンプト生成を責務別モジュールに分割し、既存の公開 API と挙動を維持するために SHALL 分割後のモジュール構成を採用しなければならない。

#### Scenario: Agent モジュール構成
- **WHEN** 開発者が Agent モジュールを調査する
- **THEN** runner/output/history/prompt の責務別モジュールが確認できる

#### Scenario: 既存の挙動維持
- **WHEN** 分割後に既存のテストを実行する
- **THEN** すべて成功し、挙動が変わっていないことが確認できる

### Requirement: 進捗取得とAPIエラー応答の共通化
オーケストレーターは change の進捗取得と Web API の Not Found 応答生成を共通ヘルパーに集約し、既存挙動を維持するために SHALL 共通ヘルパーを使用しなければならない。

#### Scenario: 進捗取得のフォールバック順序を維持する
- **WHEN** TUI または Web が change の進捗を取得する
- **THEN** 共通ヘルパーが worktree → archive → base の順でフォールバックする

#### Scenario: Not Found 応答の形式を維持する
- **WHEN** Web API が change を見つけられない
- **THEN** 共通ヘルパーが既存と同等の StatusCode とエラーメッセージを返す

