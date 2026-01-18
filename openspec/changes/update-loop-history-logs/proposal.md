# Change: ループ履歴注入とログヘッダーの整合性修正

## Why
並列 apply/archive の履歴注入が仕様に反して欠落しており、トラブル時の再試行が不安定です。また archive のデフォルト system-context が実装と運用に齟齬を生み、有害な指示になっています。さらに resolve/analysis のログ先頭表示（[resolve:N]/[analysis]）が消えており、再試行回数の把握が困難です。

## What Changes
- archive のデフォルト system-context を削除し、空の既定値にする
- 逐次/並列の apply と archive で履歴コンテキストを必ず注入する
- resolve/analysis を含む全ループログに試行番号を明示し、先頭ヘッダー表示を復旧する

## Impact
- Affected specs: cli, configuration, tui-architecture
- Affected code: src/parallel/executor.rs, src/agent.rs, src/config/defaults.rs, src/tui/*, src/parallel/*
