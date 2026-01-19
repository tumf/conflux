# Change: reanalysis trigger follows spec during parallel execution

## なぜ
現状の並列実行では、キュー変化があっても re-analysis が実行中グループ完了まで開始されず、仕様で求める「完了イベントとメイン実行ループ進行に依存しない再解析」に一致しません。実行中の変更が長い場合に応答性が低下し、仕様と実装の乖離が発生します。

## 何を変えるか
- キュー変化やタイマーをトリガにした re-analysis を、実行中グループ完了やメインの実行ループ進行を待たずに開始できるようにする。
- re-analysis の起動とスロット可否を分離し、スロットが空いていない場合でも分析は進め、空きができたタイミングでディスパッチする。
- 既存のデバウンス（キュー変化から一定時間待つ）を維持し、不要な連続再解析を抑制する。

## 影響範囲
- 対象仕様: parallel-execution
- 主な対象コード: `src/parallel/mod.rs`, `src/tui/queue.rs`, `src/parallel_run_service.rs`
