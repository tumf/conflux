# タスク: configure-agent-commands

## 1. 設定ファイルの実装

- [x] 1.1 `src/config.rs` モジュールを作成し、JSONC パーサーを実装
- [x] 1.2 設定構造体 `OrchestratorConfig` を定義（`apply_command`、`archive_command`、`analyze_command`）
- [x] 1.3 `{change_id}` と `{prompt}` プレースホルダーの展開ロジックを実装
- [x] 1.4 プロジェクト設定ファイル（`.openspec-orchestrator.jsonc`）の読み込み関数を実装
- [x] 1.5 グローバル設定ファイル（`~/.config/openspec-orchestrator/config.jsonc`）の読み込み関数を実装
- [x] 1.6 設定ファイルの優先順位ロジックを実装（プロジェクト > グローバル > デフォルト）

## 2. エージェントランナーの抽象化

- [x] 2.1 `OpenCodeRunner` を汎用的な `AgentRunner` に名前変更またはリファクタリング
- [x] 2.2 `run_command` をテンプレートベースのコマンド実行に変更
- [x] 2.3 `analyze_dependencies` を `{prompt}` プレースホルダーを使用した設定可能なコマンドに変更

## 3. オーケストレーターの統合

- [x] 3.1 `Orchestrator::new` に設定読み込みを追加
- [x] 3.2 設定が存在しない場合のフォールバックロジックを実装
- [x] 3.3 CLI に `--config` オプションを追加（オプション、カスタムパス指定用）

## 4. テストとドキュメント

- [x] 4.1 設定ファイル解析のユニットテストを追加
- [x] 4.2 プレースホルダー展開のユニットテストを追加（`{change_id}` と `{prompt}`）
- [x] 4.3 設定ファイル優先順位のユニットテストを追加
- [x] 4.4 E2E テストで異なるエージェント設定をテスト
- [x] 4.5 設定ファイルのサンプルを作成（`.openspec-orchestrator.jsonc.example`）
- [x] 4.6 README に設定ファイルの使い方を追記
