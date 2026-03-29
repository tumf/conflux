# Fix: Ready中のMキーresolve開始時にAppModeをRunningに遷移する

**Change Type**: implementation

## Problem/Context

TUIがReady状態（`AppMode::Select`）のとき、MergeWait変更にMキーを押してresolveを開始すると、resolveタスクはバックグラウンドで実行されるが、`app.mode`が`AppMode::Select`のまま変わらない。ステータスバーは「Ready」表示のままとなり、ユーザーにはresolveが実行中であることが分からない。

F5キー（`start_processing`/`resume_processing`）では正しく`AppMode::Running`に遷移している。

## Root Cause

`src/tui/state.rs` の `resolve_merge()` メソッド（L873-877）で、`is_resolving == false`（resolve未実行）の分岐において `TuiCommand::ResolveMerge` を返す際に `self.mode = AppMode::Running` への遷移が欠落している。

## Proposed Solution

`resolve_merge()` 内で、resolveが未実行（`!self.is_resolving`）かつ現在のモードが `AppMode::Select` または `AppMode::Stopped` の場合に、`self.mode = AppMode::Running` へ遷移する。既に`Running`モードの場合は変更不要。

## Acceptance Criteria

- Ready（Select）中にMergeWait変更にMキーを押すと、ステータスバーが「Running」に変わる
- Stopped中にMergeWait変更にMキーを押すと、ステータスバーが「Running」に変わる
- Running中にMergeWait変更にMキーを押しても、モードは`Running`のまま（既存動作を壊さない）
- resolveが既に実行中（`is_resolving == true`）の場合はモード遷移しない（キューに追加するだけ）

## Out of Scope

- `is_resolving == true` 時のキュー追加フローの変更
- F5キーの動作変更
