# Change: Update log iteration headers

## Why
TUIログのアーカイブ出力でイテレーション番号が欠落し、どのリトライ出力か判別できないケースがある。さらに change_id のない analysis ログでもイテレーション番号が省略されると再実行が区別できないため、ヘッダー表記を一貫させる必要がある。

## What Changes
- archive ログ出力に必ずイテレーション番号を付与し、`[change_id:archive:N]` 形式で表示する。
- change_id のない analysis ログにイテレーション番号を必須化し、`[analysis:N]` 形式で表示する。
- 直列/並列の両経路で同じルールを適用し、ログヘッダーの表示仕様を統一する。

## Impact
- Affected specs: tui-architecture
- Affected code: src/tui/orchestrator.rs, src/parallel/executor.rs, src/tui/state/events.rs, src/web/state.rs
