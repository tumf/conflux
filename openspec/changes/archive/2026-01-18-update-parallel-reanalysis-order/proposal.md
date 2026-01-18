# Change: 並列再分析を順序リスト方式に更新

## Why
並列実行の効率を高めるため、実行スロットに空きが出た際の再分析を「グループ単位」ではなく「順序付きの候補リスト」へ変更し、空き数に応じて即時に次の変更を起動できるようにする必要がある。

## What Changes
- 依存関係分析の出力形式を `groups` から `order` に変更する
- 再分析のタイミングを 10 秒間隔に統一し、キュー追加/削除でタイマーをリセットする
- 実行スロットの空き数に応じて `order` から実行候補を選び、依存関係が解決済みのものだけ空き数分起動する
- 依存関係は制約として扱い、依存先が merged になるまで起動しない
- 依存解決後の実行開始時点で worktree を新規作成し、既存の worktree があれば再作成する
- CLI/TUI 共通の parallel 実行フローに適用する

## Impact
- Affected specs: parallel-analysis, parallel-execution
- Affected code: src/analyzer.rs, src/parallel/mod.rs, src/parallel_run_service.rs, src/orchestrator.rs
