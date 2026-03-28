# Change: Resolving 中に TUI が Ready に遷移し、他 change のキュー追加ができなくなるバグを修正

**Change Type**: implementation

## Why

Resolve タスクは `tokio::spawn` でバックグラウンド実行されるが、オーケストレーションループ（serial/parallel）は resolve の完了を待たず `AllCompleted` を送信する。`handle_all_completed()` は Resolving 状態の change を考慮せず `AppMode::Select`（Ready）に遷移するため、以下の 2 つの問題が発生する：

1. Resolving 中にヘッダーが「Ready」と表示される（実際にはまだ処理中）
2. Select モードでは Space キーが `selected` フラグのみ反転し `AddToQueue` コマンドを発行しないため、他の change をキューに追加できない

## What Changes

- `handle_all_completed()` で Resolving 中の change がある場合、`AppMode::Running` を維持する
- Resolve 完了/失敗時に全 active change がゼロなら `AppMode::Select` へ遷移するロジックを追加
- `handle_stopped()` のリセット対象に `QueueStatus::Resolving` を追加

## Impact

- Affected specs: tui-architecture
- Affected code: `src/tui/state.rs` (`handle_all_completed`, `handle_resolve_completed`, `handle_resolve_failed`, `handle_stopped`)
