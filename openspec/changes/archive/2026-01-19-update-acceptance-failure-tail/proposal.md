# Change: Acceptance failure tail lines

## Why
受理判定が FAIL の場合、FINDINGS 形式の抽出に失敗すると tasks.md に 0 件として記録され、apply へ適切な情報が戻らない状況が発生します。受理出力の末尾をそのまま引き継ぐことで、形式揺れに強い失敗情報の伝達を実現します。

## What Changes
- 受理結果が FAIL または受理コマンド失敗の場合、FINDINGS 抽出に依存せず stdout/stderr の末尾 N 行を tasks.md に追記する
- 末尾 N 行は既存の受理出力テールの設定に合わせ、既定の収集行数を利用する
- CLI/TUI/並列実行で同一の失敗情報収集ルールを適用する

## Impact
- Affected specs: cli, parallel-execution
- Affected code: src/acceptance.rs, src/orchestration/acceptance.rs, src/parallel/executor.rs
