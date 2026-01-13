# observability Specification

## Purpose
システムの動作を可視化し、デバッグとトラブルシューティングを支援するための観測可能性機能を定義する。

## ADDED Requirements

### Requirement: REQ-OBS-001 すべてのコマンド実行のログ記録

オーケストレーターは外部コマンド（`tokio::process::Command`, `std::process::Command`）を実行する前に、コマンド情報をログに記録しなければならない (MUST)。

ログには以下の情報を含めなければならない：
- 実行可能ファイル名
- 引数リスト
- 作業ディレクトリ（設定されている場合）

#### Scenario: VCSコマンド実行時のログ出力

- **GIVEN** git worktreeを作成する
- **WHEN** `git worktree add` コマンドが実行される
- **THEN** ログに `debug!` レベルでコマンドライン全体が記録される
- **AND** ログに作業ディレクトリが含まれる

#### Scenario: AIエージェントコマンド実行時のログ出力

- **GIVEN** changeをapplyする
- **WHEN** OpenCodeエージェントコマンドが実行される
- **THEN** ログに `info!` レベルでコマンドラインが記録される

#### Scenario: フック実行時のログ出力

- **GIVEN** on_apply_startフックが設定されている
- **WHEN** フックコマンドが実行される
- **THEN** ログに `info!` レベルでコマンドラインが記録される
- **AND** ログに "Running on_apply_start hook" というコンテキストが含まれる

### Requirement: REQ-OBS-002 適切なログレベル分類

オーケストレーターはコマンドの重要度に応じて適切なログレベルを使用しなければならない (MUST)。

ログレベルの基準：
- `info!`: ユーザー向けの主要操作（apply, archive, analyze, hooks実行）
- `debug!`: 内部的なVCSコマンド、補助的なコマンド実行

#### Scenario: デフォルトログレベルでの出力制御

- **GIVEN** RUST_LOG環境変数が設定されていない（デフォルト）
- **WHEN** orchestratorを実行する
- **THEN** `info!` レベルのコマンドログが表示される
- **AND** `debug!` レベルのVCSコマンドログは表示されない

#### Scenario: デバッグモードでの詳細ログ出力

- **GIVEN** RUST_LOG=debug が設定されている
- **WHEN** orchestratorを実行する
- **THEN** すべてのVCSコマンドログが表示される
- **AND** 内部的な補助コマンドのログも表示される

### Requirement: REQ-OBS-003 統一されたログフォーマット

オーケストレーターは一貫性のあるログフォーマットを使用しなければならない (MUST)。

フォーマット規則：
- コマンド実行前: `"Running {context}: {command}"` または `"Executing {command}"`
- コンテキスト情報を可能な限り含める（例：change ID, workspace path）

#### Scenario: 統一フォーマットでのログ出力

- **GIVEN** 複数の種類のコマンドが実行される
- **WHEN** ログを確認する
- **THEN** すべてのコマンドログが統一されたフォーマットで出力されている
- **AND** コンテキスト情報（change ID等）が含まれている

#### Scenario: 長いコマンドラインの扱い

- **GIVEN** 非常に長い引数を持つコマンドを実行する
- **WHEN** ログを確認する
- **THEN** コマンドライン全体が記録されている（切り詰めない）

## Related Specifications

- `code-maintenance`: ログ追加によるコード品質維持
- `testing`: ログ出力の検証テスト
- `parallel-execution`: 並列実行時のVCSコマンドログ
- `hooks`: フック実行時のログ
