# Change: F5 (start_processing) が Blocked 状態の変更を Queued にリセットして再ディスパッチするバグの修正

## Why

TUI で変更 A(Applying), B(Blocked), C(Blocked) の状態で実行中に新しい変更を追加して F5 を押すと、Blocked 状態の B, C が Queued にリセットされ、依存先の A が完了していないにもかかわらず Applying に遷移してしまう。

原因は2箇所:

1. `start_processing()` のフィルタが `MergeWait` / `ResolveWait` のみ除外し、`Blocked` / `Merged` / `Error` 等のアクティブ・終端状態を除外していない
2. `handle_stopped()` / `handle_all_completed()` が `Blocked` を `NotQueued` にリセットしないため、Select モードに戻っても `Blocked` + `selected=true` のまま残り、次の F5 で再送される

## What Changes

- `start_processing()` のフィルタにホワイトリスト方式を導入: `NotQueued` の変更のみを `Queued` に遷移させる
- `handle_stopped()` と `handle_all_completed()` の reset 対象に `Blocked` を追加
- 回帰テストを追加

## Impact

- Affected specs: `parallel-execution`（Dependent Change Skipping の状態遷移に関連）
- Affected code:
  - `src/tui/state.rs` の `start_processing()`（行 983-988, 1022-1031）
  - `src/tui/state.rs` の `handle_stopped()`（行 1540-1556）
  - `src/tui/state.rs` の `handle_all_completed()`（行 1515-1518）
