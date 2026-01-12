# Change: execution モジュールの基盤作成

## Why

現在、serial mode と parallel mode で同じ目的のロジックが別々に実装されており、コードの重複と2重管理の問題が発生している。特に：

1. **Archive ロジック**: `tui/orchestrator.rs::archive_single_change` と `parallel/executor.rs::execute_archive_in_workspace` で約180行 vs 290行の重複
2. **Apply ロジック**: 反復処理、進捗チェック、ストリーミング出力が複数箇所で重複
3. **VCS 操作**: git/jj コマンドが各所で直接呼び出されており、既存の `WorkspaceManager` 抽象化が活用されていない
4. **Hooks サポート**: Parallel mode では hooks が未サポート（機能ギャップ）

これらを解消するため、共通実行ロジックを格納する `src/execution/` モジュールの基盤を作成する。

## What Changes

- 新規: `src/execution/mod.rs` - モジュールルート
- 新規: `src/execution/types.rs` - 共通型定義（`ExecutionContext`, `ExecutionResult`）
- 新規: `src/main.rs` に `mod execution;` を追加

## Impact

- Affected specs: code-maintenance
- Affected code:
  - 新規: `src/execution/mod.rs`
  - 新規: `src/execution/types.rs`
  - 修正: `src/main.rs`
- **BREAKING**: なし（新規モジュール追加のみ）

## 依存関係

この変更は以下の変更提案の前提条件となる：
- `refactor-archive-common` - アーカイブロジックの共通化
- `refactor-apply-common` - Apply ロジックの共通化
- `add-parallel-hooks` - Parallel への hooks サポート追加
