# Change: Streaming 実行のリトライ有効化

## Why
Streaming 実行経路でリトライが適用されず、ENOENT など一時的な失敗でも即座に apply が失敗してしまうため、既存のリトライ仕様と実装が乖離しています。

## What Changes
- streaming 実行経路でも command queue のリトライ判定を適用する
- リトライ通知を出力チャネルに送信し、再実行の進捗を可視化する

## Impact
- Affected specs: command-queue
- Affected code: src/agent.rs（streaming 実行の呼び出し経路）
