# Change: sequenceモード実行中にキューから削除したchangeが実行されてしまう問題の修正

## Why

sequenceモード（連続実行モード）の実行中に、ユーザーが queued 状態のchangeのマーカーを [x] から [@] に変更してキューから削除しても、実行キュー（DynamicQueue）から削除されないため、そのchangeが実行されてしまうバグがある。

これにより、ユーザーが意図的にキューから外したchangeが予期せず実行され、望まない変更が適用される可能性がある。

## What Changes

- DynamicQueueに `remove` メソッドを追加し、キューから特定のchange IDを削除できるようにする
- `TuiCommand::UnapproveAndDequeue` 処理時に、DynamicQueueからも該当changeを削除する
- Spaceキーでキューから外す際にも、DynamicQueueから削除する
- 削除機能のテストを追加する

## Impact

- Affected specs: `specs/tui-architecture/spec.md`
- Affected code:
  - `src/tui/queue.rs` - DynamicQueueに `remove` メソッドを追加
  - `src/tui/runner.rs` - dequeue処理でDynamicQueueからも削除
  - `src/tui/state/mod.rs` - Spaceキーでの削除時にDynamicQueueからも削除
  - `src/tui/orchestrator.rs` - dynamic_queue参照の渡し方を調整（必要に応じて）
