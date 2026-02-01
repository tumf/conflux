# Change: CommandQueue リトライ判定の共通化

## Why
ストリーミング/非ストリーミングのリトライ処理が重複しており、修正時の漏れが発生しやすい状態です。

## What Changes
- リトライ判定ロジックを共通ヘルパーに集約する
- 既存の判定条件（エラーパターン/実行時間/exit code）と挙動を維持する

## Impact
- Affected specs: command-queue
- Affected code: src/command_queue.rs
