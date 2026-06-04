---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/render.rs
  - openspec/specs/code-maintenance/spec.md
---

# TUI rendering の描画責務分割

**Change Type**: implementation

## Problem / Context

`src/tui/render.rs` は 3,600 行超の大型ファイルで、選択画面、実行画面、header/status/logs/footer、worktree view、popup、広範な snapshot-like unit tests が同居している。描画関数の一覧だけでも `render_select_mode`、`render_running_mode`、`render_changes_list_select`、`render_changes_list_running`、`render_logs`、`render_worktree_view`、各 popup があり、UI の小変更で関係ない描画領域まで読み込む必要がある。

証拠:

- `src/tui/render.rs:203` に TUI 全体の `render` entry point がある。
- `src/tui/render.rs:426` と `src/tui/render.rs:730` に select/running の changes list 描画が分かれている。
- `src/tui/render.rs:1290` に logs 描画、`src/tui/render.rs:1537` に worktree view 描画がある。
- `src/tui/render.rs:1978` 以降に多数の rendering tests が同一ファイル内に存在する。

## Proposed Solution

表示挙動を変えずに、TUI rendering を領域別の内部モジュールへ分割する。先に主要画面の characterization test を固定し、entry point と public function は維持したまま、changes list、status/logs、worktree、popup、test helpers を整理する。

## Acceptance Criteria

- TUI の選択画面、実行画面、worktree view、warning/QR popup の表示内容と key hints は変更されない。
- remote change grouping、selected row、status/log 表示、worktree delete confirmation の代表 test が維持される。
- `render(frame, app)` の public entry point は維持される。
- `cargo test tui::render` が成功する。
- UI 仕様や操作フローの変更は含まれない。

## Explicit Completion Conditions

- `src/tui/render.rs` の entry point は残しつつ、描画領域ごとの実装が小さな module/function へ分かれている。
- 既存 rendering tests または追加 characterization tests が、主要画面の表示同等性を確認している。
- `cargo test tui::render` が成功する。

## Out of Scope

- TUI デザイン変更。
- キーバインド変更。
- 状態管理 `AppState` の構造変更。
- WebUI dashboard の変更。
