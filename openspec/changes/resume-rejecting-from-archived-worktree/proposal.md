---
change_type: implementation
priority: high
dependencies:
  - classify-acceptance-followup-routing
references:
  - src/orchestration/rejection.rs
  - src/task_parser.rs
  - src/parallel/dispatch.rs
  - src/execution/archive.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# Change: archived worktree からの rejecting resume を active-path 前提にしない

**Change Type**: implementation

## Premise / Context

- `.last-checked` 以降の cflx 実行ログでは `refactor-split-tui-state-appstate` が rejection review で `RESUME` を返した直後、`Failed to read tasks.md while resuming apply ... No such file or directory` で error 終了していた（`~/.local/state/cflx/logs/conflux-bda270b8/2026-04-29.log:358255-358280`）。
- 同じ時点で worktree state 判定は `WorkspaceState::Archived` を返しており、change directory は消えて archive entry は存在していた（`src/execution/state.rs` の観測ログ、および `src/tui/runner.rs` の MergeWait 復元ロジックと整合）。
- しかし rejecting resume の実装は `openspec/changes/<change_id>/tasks.md` の active path だけを読み書きしており、archive 済み worktree の `openspec/changes/archive/<date>-<change_id>/tasks.md` を見ない（`src/orchestration/rejection.rs`）。
- 一方、task progress 読み取り系はすでに worktree archive location fallback を持っており、archived workspace でも進捗を読める設計になっている（`src/task_parser.rs`）。
- したがって今回の error は change 自体の正当な reject ではなく、resume path が archived workspace を扱えていないコア不具合とみなせる。

## Requested Artifact

- implementation proposal for rejection-review resume/block handling on archived workspaces
- canonical contract clarifying that rejecting recovery updates must target the active change dir or the archived entry whichever currently exists
- regression coverage for archived-workspace resume paths

## Problem / Context

rejection review が `RESUME` または `BLOCK` を返した時、runtime は recovery task を tasks.md に追記して `REJECTED.md` を消す。しかし現在の実装は active change directory の `openspec/changes/<change_id>/tasks.md` に固定されているため、archive が既に完了した worktree では tasks.md が見つからず、そのまま terminal `Error` へ落ちる。

この失敗は「resume 不能な change」ではなく、「resume metadata の書き込み先解決が archived state を考慮していない」ことが原因である。特に archive 後に rejection review が走りうる parallel lifecycle では、active path 前提のままでは同種エラーが再発する。

## Proposed Solution

rejecting recovery の tasks 書き込み先を workspace state aware にし、active change dir が無い場合は archive entry の tasks.md を更新できるようにする。

- `src/orchestration/rejection.rs` に active/archived 両対応の tasks path resolver を追加し、`append_recovery_task()` が worktree 内の現在存在する canonical tasks.md を選ぶ。
- archive 済み change に recovery task を追記した場合でも、`parse_progress_with_fallback()` と整合する path/format を維持する。
- rejection review の `RESUME` / `BLOCK` どちらでも archived workspace で失敗せず、change を `Applying` または `Blocked` に遷移させる。
- ログとエラーメッセージを改善し、active path missing だけで即 failure にせず、どの path を探索したかを観測できるようにする。
- archived workspace からの rejecting resume regression test を追加し、active path 不在でも archive path を使って recovery task が更新されることを固定する。

## Acceptance Criteria

- rejection review が `RESUME` を返した archived workspace で、runtime は `openspec/changes/<change_id>/tasks.md` 不在を理由に error 終了しない。
- active change dir が存在しない場合、rejecting recovery 更新は worktree archive entry の `tasks.md` を対象にできる。
- rejection review が `BLOCK` を返した archived workspace でも同じ path resolution を用い、change は `Blocked` に戻る。
- tasks path resolution failure が起きる場合、探索した active path / archive path が error message に含まれる。
- regression tests が archived workspace の `RESUME` / `BLOCK` 両方で recovery task 追記を確認する。

## Explicit Completion Conditions

- `openspec/specs/parallel-execution/spec.md` または `openspec/specs/orchestration-state/spec.md` に、rejecting recovery が archived workspace の tasks location も扱う canonical requirement が追加されている。
- `src/orchestration/rejection.rs` の recovery task 更新先解決が active-only ではなく archive fallback を持つ。
- `src/parallel/dispatch.rs` 経由の rejection review resume/block path を再現する Rust tests が tasks に含まれている。
- `cflx openspec validate resume-rejecting-from-archived-worktree --strict --evidence warn` が成功する。

## Out of Scope

- archive root-cause reporting 全体の再設計
- acceptance follow-up taxonomy そのものの再定義
- merge wait / resolve wait lifecycle の別件改善
