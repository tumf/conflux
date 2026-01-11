# Change: TUI 状態管理モジュールの分割

## Why

`tui/state.rs` は 1358 行と巨大で、以下の責務が混在している：

1. `AppState` 構造体（UI 状態の管理）
2. `ChangeState` 構造体（変更の状態表現）
3. モード切替ロジック
4. ログ管理
5. オーケストレーターイベントのハンドリング
6. 大量のテストコード（32 個のテスト関数）

状態管理とビジネスロジックが密結合しており、テストや変更が困難。

## What Changes

- `tui/state.rs` を責務ごとにサブモジュールへ分割
  - `tui/state/mod.rs` - AppState 本体
  - `tui/state/change.rs` - ChangeState
  - `tui/state/modes.rs` - モード関連ロジック
  - `tui/state/logs.rs` - ログ管理
  - `tui/state/events.rs` - イベントハンドリング
- テストを各モジュールに分散

## Impact

- 対象 specs: `code-maintenance`
- 対象コード:
  - `src/tui/state.rs` → `src/tui/state/` ディレクトリに分割
  - `src/tui/render.rs` - インポートパスの更新
  - `src/tui/runner.rs` - インポートパスの更新
