# Change: プロダクト名を Conflux にリネーム、CLIコマンドを cflx に変更

## Why

現在の `openspec-orchestrator` という名前は:
- 長くてタイプしづらい
- 製品としてのブランディングが弱い
- OpenSpec の一部機能と誤解されやすい

より短く覚えやすい名前に変更することで、ユーザビリティとブランド認知を向上させる。

## What Changes

**BREAKING CHANGES:**

- **製品名**: `OpenSpec Orchestrator` → `Conflux`
- **CLIコマンド名**: `openspec-orchestrator` → `cflx`
- **Rustパッケージ名**: `openspec-orchestrator` → `conflux`
- **設定ファイル名**: `.openspec-orchestrator.jsonc` → `.cflx.jsonc`
- **グローバル設定ディレクトリ**: `~/.config/openspec-orchestrator/` → `~/.config/cflx/`
- **後方互換性**: 旧設定ファイル名は読み込まない（破壊的変更）

## Impact

- Affected specs:
  - `cli` - コマンド名とヘルプメッセージ
  - `configuration` - 設定ファイルパスと優先順位
- Affected code:
  - `Cargo.toml` - パッケージ名とバイナリ名
  - `src/cli.rs` - CLI定義とテスト
  - `src/config/defaults.rs` - 設定ファイル名定数
  - `src/config/mod.rs` - 設定読み込みロジック
  - `src/main.rs` - `init` コマンドの生成パス
  - `src/orchestrator.rs` - ユーザー向けメッセージ
  - `src/templates.rs` - 設定テンプレート
  - すべてのテスト - CLI名の参照
- Affected docs:
  - `README.md`, `README.ja.md` - 実行例と設定パス
  - `DEVELOPMENT.md` - 開発者向けドキュメント
  - `AGENTS.md` - AI向けドキュメント

## Migration Guide

既存ユーザーは以下の手順で移行:

1. 設定ファイルをリネーム:
   ```bash
   mv .openspec-orchestrator.jsonc .cflx.jsonc
   ```

2. グローバル設定をリネーム:
   ```bash
   mv ~/.config/openspec-orchestrator ~/.config/cflx
   ```

3. コマンド実行を更新:
   ```bash
   # 旧: openspec-orchestrator run
   # 新: cflx run
   ```

4. フック・スクリプト内の参照を更新
