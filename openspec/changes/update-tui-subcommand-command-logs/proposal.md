# Change: TUIでサブコマンドのコマンド表示を統一

## Why
TUIのログでapply以外のサブコマンド（archive/acceptance/resolve）のコマンド文字列が表示されず、実行内容の追跡が困難になっています。すべてのサブコマンドでコマンド表示が揃うように、イベントとログ出力の仕様を明確化します。

## What Changes
- TUIのログに、apply/archive/acceptance/resolveの開始時コマンド文字列を必ず表示する
- サブコマンドの出力ログは対応するoperationで記録される

## Impact
- Affected specs: specs/tui-architecture/spec.md
- Affected code: src/events.rs, src/tui/orchestrator.rs, src/serial_run_service.rs, src/parallel/executor.rs
