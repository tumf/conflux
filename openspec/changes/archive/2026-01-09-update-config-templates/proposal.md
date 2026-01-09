# Change: 設定テンプレートの構造修正と完全化

## Why

現在の `templates.rs` のテンプレートには以下の問題がある：

1. **構造の不一致**: テンプレートは `agent.apply_command` のネスト形式だが、`config.rs` はフラットな構造（`apply_command` 直接）を期待している
2. **コマンドの欠落**: `archive_command` と `analyze_command` がテンプレートに含まれていない
3. **出力オプションの欠落**: Claude Code の `--verbose --output-format stream-json` オプションが含まれていない

## What Changes

- テンプレートの構造を `config.rs` の `OrchestratorConfig` と一致させる
- 全てのコマンド（`apply_command`, `archive_command`, `analyze_command`）を含める
- Claude テンプレートに適切な出力オプションを追加

### Affected Specs
- `configuration`: ADDED requirements for template structure and Claude command options

## Scope

- **In scope**: `templates.rs` のテンプレート内容更新
- **Out of scope**: `config.rs` の構造変更（現在の実装が正しい）

## Impact

- **Low risk**: テンプレート内容の修正のみ
- **Breaking change**: 既存の `init` で生成された設定ファイルが古い形式の場合、手動更新が必要
