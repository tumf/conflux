---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/parallel-merge/spec.md
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/orchestration-state/spec.md
  - src/parallel/merge.rs
  - src/tui/command_handlers.rs
  - src/tui/runner.rs
---

**Change Type**: implementation

# Change: archive 後の merge 判定を簡素化し MergeWait stuck を修正

## Why

archive 完了後の merge 試行で base が dirty な場合、dirty の reason 文字列から auto_resumable かどうかを推測する `is_dirty_reason_auto_resumable()` を使っている。しかし resolve 処理自体が base に uncommitted changes を一時的に作るため、MERGE_HEAD がないタイミングでは「ユーザー起因の dirty」と誤分類され `auto_resumable=false` → MergeWait（手動待ち）に落ちる。

実際にはプロジェクトレベルの resolve カウンター（`auto_resolve_count` + `manual_resolve_count`、TUI 側は `resolve_counter`）を先にチェックしており、resolve 中なら dirty check に到達する前に弾かれるはず。つまり dirty check に到達した時点で resolve は動いていないので、dirty は必ずユーザー起因。

加えて `MergeDeferred` イベントが TUI runner の `apply_to_reducer` に含まれていないため、reducer に反映されず次の `ChangesRefreshed` で MergeWait が上書きされ M キーが消える二次バグがある。

## What Changes

- `is_dirty_reason_auto_resumable()` を削除し、dirty check に到達した場合は常に `auto_resumable=false` とする
- `MergeDeferred` イベント処理で auto_resumable 分岐を reason 文字列解析から切り離し、resolve カウンターによる判定のみに依存する
- TUI runner の `apply_to_reducer` に `MergeDeferred` を追加して reducer 同期を保証する

## Impact

- Affected specs: parallel-merge, tui-resolve, orchestration-state
- Affected code: `src/parallel/merge.rs`, `src/tui/command_handlers.rs`, `src/tui/runner.rs`
- 関連テストの更新: `is_dirty_reason_auto_resumable` を使うテストの削除・修正

## Out of Scope

- resolve 処理自体のロジック変更
- MergeWait からの手動 resolve フロー（既に正しく動作する前提）
