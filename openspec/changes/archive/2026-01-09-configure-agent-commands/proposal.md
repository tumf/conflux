# Change: エージェントコマンドの設定可能化

## Why

現在のオーケストレーターは OpenCode に固定されており、他のエージェントCLIツール（Codex、Claude Code など）を使用できない。これにより、ユーザーは好みのAIツールを選択できず、オーケストレーターの汎用性が制限されている。

## What Changes

- JSONC 形式の設定ファイル（`.opencode/orchestrator.jsonc`）を導入
- `apply`、`archive`、`analyze_dependencies` コマンドをテンプレート形式で設定可能に
- `{change_id}` プレースホルダーによる動的な変更ID挿入をサポート
- 設定がない場合は現在の OpenCode 動作にフォールバック
- オーケストレーターを真にエージェント非依存に

## Impact

- 影響する仕様: `configuration`
- 影響するコード:
  - `src/opencode.rs` - エージェントランナーの抽象化
  - `src/orchestrator.rs` - 設定の読み込みと適用
  - 新規: `src/config.rs` - 設定ファイルの解析
