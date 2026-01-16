# Proposal: AI エージェントクラッシュリカバリー

## Summary

AI エージェント（OpenCode, Claude Code など）が Apply または Archive コマンド実行中にクラッシュした場合、システムが自動的にリトライして回復する機能を追加する。

## Motivation

### 現状の問題

1. **AI エージェントのクラッシュで即座に失敗**: OpenCode などの AI エージェントがクラッシュ（異常終了）すると、Orchestrator は即座にエラーを返却し、処理を中断する
2. **一時的なエラーからの回復不能**: ネットワーク問題、リソース競合、AI サービスの一時的な障害など、再試行で解決可能な問題でも処理が停止する
3. **ユーザー介入が必要**: 一時的なエラーでも、ユーザーが手動で再実行する必要がある

### 具体例

```
Error: No such file or directory
JSON Parse error: Unexpected EOF
Archive failed for rename-to-conflux
```

上記のようなエラーが発生した場合：
- OpenCode が内部で `mv` コマンドを実行しようとして失敗
- 不完全な JSON を出力してクラッシュ
- Orchestrator は即座にエラーを返却（リトライなし）

### 既存のリトライ機構との関係

`command-queue` の自動リトライ機構は、特定のエラーパターン（例: `Cannot find module`）または短時間での失敗を検出してリトライする。しかし、これは **コマンド spawn 後の出力パース段階** でのリトライであり、**コマンド自体が異常終了（exit code ≠ 0）した場合のリトライ**は対象外。

## Solution

### 概要

Apply / Archive コマンドが異常終了（`!status.success()`）した場合、即座にエラーを返却せず、設定された回数までリトライする。

### 設計

- **リトライ回数**: 2回（既存の `ARCHIVE_COMMAND_MAX_RETRIES` と一致）
- **待機時間**: 2秒（既存の `command_queue_stagger_delay_ms` と一致）
- **リトライ対象**: 全てのエラー（exit code ≠ 0）
- **エラー情報の引き継ぎ**: なし（シンプルな実装）

### 対象コード

- `src/parallel/executor.rs`:
  - `execute_apply_in_workspace()` 関数（Apply コマンド）
  - `execute_archive_in_workspace()` 関数（Archive コマンド）

## Scope

### In Scope

- Apply コマンドのクラッシュリカバリー
- Archive コマンドのクラッシュリカバリー
- リトライ時のログ出力

### Out of Scope

- エラー情報の履歴管理・AI への引き継ぎ（将来の拡張）
- Analyze / Resolve コマンドのリトライ（既存の仕組みで対応済み）
- 設定ファイルでのリトライ設定カスタマイズ（将来の拡張）

## Risk

- **無限リトライのリスク**: 最大リトライ回数で制限（2回）
- **永続的エラーへの遅延**: リトライにより失敗報告が遅れる可能性があるが、待機時間は短い（2秒）ため影響は最小限
