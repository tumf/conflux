# Change: 全仕様の Purpose フィールドを更新

## Why

12個の仕様ファイルで Purpose フィールドが「TBD - created by archiving change...」のまま放置されている。各仕様の目的を明確に記述することで、仕様の理解とメンテナンスが容易になる。

## What Changes

以下の仕様の Purpose を適切な説明に更新:
- cli: CLI コマンドとサブコマンドの仕様
- configuration: 設定ファイルの形式と読み込み
- documentation: README とドキュメントの要件
- hooks: ライフサイクルフックシステム
- parallel-execution: 並列実行機能
- release-workflow: リリースプロセス
- testing: テスト戦略と要件
- tui-architecture: TUI モジュール構造
- tui-editor: TUI エディタ統合
- tui-key-hints: TUI キーバインドヒント
- workspace-cleanup: ワークスペースクリーンアップ
- code-maintenance: コード保守ガイドライン

## Impact

- Affected specs: 全12仕様（メタデータのみ）
- Affected code: なし

**注意**: これは仕様の要件（Requirements）の変更ではなく、メタデータの更新のため、spec delta は作成しない。アーカイブ時は `openspec archive update-spec-purpose-fields --skip-specs --yes` を使用すること。
