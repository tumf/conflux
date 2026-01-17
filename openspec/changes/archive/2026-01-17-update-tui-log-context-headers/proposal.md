# Change: TUIログのコンテキストヘッダー拡張

## Why
TUIのログ表示でapplyのイテレーションが常に1になる/表示されないケースがあり、archiveやresolveの詳細を追えません。ensure_archive_commitやanalysis/resolveの処理単位を明確にするため、ログヘッダーのコンテキスト表示を拡張します。

## What Changes
- TUIログのヘッダーにarchiveのイテレーション番号を表示する
- ensure_archive_commit由来のログを専用オペレーション名で表示する
- analysis/resolveのログにイテレーション番号を表示する
- 既存ログの後方互換表示を維持する

## Impact
- Affected specs: tui-architecture
- Affected code: src/events.rs, src/tui/state/events.rs, src/tui/render.rs, src/parallel/executor.rs, src/parallel/mod.rs, src/parallel/conflict.rs, src/parallel_run_service.rs, src/tui/orchestrator.rs
