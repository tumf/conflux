## Context
TUIログはchange_id/operation/iterationをヘッダーに含めて表示できるが、archive/ensure_archive_commit/analyze/resolveでは情報が不足している。処理単位の可視性を高めるため、イベント構造と表示を拡張する。

## Goals / Non-Goals
- Goals:
  - archiveの再試行回数をログヘッダーに表示する
  - ensure_archive_commitのログを独立したoperationとして可視化する
  - analysis/resolveのログにイテレーション番号を表示する
  - 既存ログ表示の互換性を維持する
- Non-Goals:
  - ログの保存形式変更
  - 新しいログビューやフィルタ機能の追加

## Decisions
- Decision: ArchiveOutput/AnalysisOutput/ResolveOutputにiterationを追加する
  - Rationale: 既存のLogEntry構造に揃え、TUI表示を一貫させる
- Decision: ensure_archive_commitは operation 名を "ensure_archive_commit" として表示する
  - Rationale: archive本体の出力と区別し、再試行の文脈を明確にする
- Decision: analysis/resolveのiterationは「実行回数」として扱う
  - Rationale: 再解析・再解決の回数を追跡できる

## Risks / Trade-offs
- イベント型の拡張によりTUI/parallel間の伝播箇所が増える
- 旧テストの期待値が変更になるため更新が必要

## Migration Plan
1. ExecutionEvent/LogEntryのフィールド拡張
2. 出力イベント送信の更新（archive/ensure_archive_commit/analysis/resolve）
3. 表示とテストの更新

## Open Questions
- analysisのiteration増分タイミングは「再解析開始時」で良いか
- resolveのiterationはグループ全体の試行回数で良いか、change単位で分けるか
