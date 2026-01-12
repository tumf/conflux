# Change: Parallel実行でProcessingStartedを送信

## Why
parallel apply開始後もTUI上でchangeが`[queued]`のまま見えるため、処理開始の可視性が不足している。processing開始を早期に示すイベントを送信し、進行中であることを即座に反映する。

## What Changes
- parallel executorがワークスペース作成/再利用時に`ExecutionEvent::ProcessingStarted`を発行する
- TUIがparallel実行でも早期にprocessing表示へ遷移する

## Impact
- Affected specs: `parallel-execution`
- Affected code: `src/parallel/executor.rs`, `src/tui/state/events.rs`
