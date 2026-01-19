# Change: re-analysis 並行化の実装指示を明確化

## なぜ
- apply 実行中に re-analysis が停止しやすく、「空きスロットで即時起動」要件を満たす実装に至れない。
- dispatch の await と in-flight 管理の責務境界が曖昧で、実装者が再分析ループの継続条件を判断しづらい。
- 仕様レベルでスケジューラの役割・トリガ・状態管理を明確化し、実装可能性を高める必要がある。

## 何を変えるか
- re-analysis ループをブロックしない非同期 dispatch の前提を明記する。
- re-analysis の起動トリガ（キュー通知 / デバウンス / in-flight 完了）を仕様に追加する。
- queued 取り込み → analysis → dispatch の順序と in-flight 追跡によるスロット算出を明文化する。

## 影響範囲
- `parallel-execution` の re-analysis 仕様
- `ParallelExecutor` のスケジューリング設計指針
- re-analysis / dispatch のログと検証観点
