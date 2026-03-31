---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/orchestration/state.rs
  - src/orchestration/acceptance.rs
  - src/serial_run_service.rs
  - src/parallel/dispatch.rs
  - src/openspec.rs
  - src/web/state.rs
  - dashboard/src/api/types.ts
---

# Change: Acceptance Blocked を Rejected 終端フローとして実装する

**Change Type**: implementation

## Why

現在 `AcceptanceResult::Blocked` はエラーとして扱われ、ワークスペースが保持されたまま終了する。しかし Blocked は「仕様レベルの差し戻し」であり、リトライで解決する問題ではない。仕様がダメと判定されたので、base ブランチまで差し戻し、理由を記録し、正常系と同様に resolve → worktree 削除まで完了させる必要がある。

## What Changes

- `TerminalState` に `Rejected` バリアントを追加し、error とは明確に区別する
- Blocked 検出後に rejection フロー（REJECTED.md 生成 → base コミット → resolve → worktree 削除）を実行する
- `list_changes_native()` で `REJECTED.md` が存在する change をスキップし、再 queue を防ぐ
- reducer の `AddToQueue` コマンドで `TerminalState::Rejected` を permanent terminal として扱い、再 queue を防ぐ
- TUI / Web / Dashboard に `"rejected"` ステータスを表示する

## Impact

- Affected specs: `orchestration-state`, `parallel-execution`
- Affected code:
  - `src/orchestration/state.rs` - TerminalState::Rejected 追加、reducer ガード
  - `src/orchestration/rejection.rs` (new) - rejection フローロジック
  - `src/serial_run_service.rs` - Blocked → rejection フロー呼び出し
  - `src/parallel/dispatch.rs` - Blocked → rejection フロー呼び出し
  - `src/openspec.rs` - REJECTED.md 存在チェックで change をスキップ
  - `src/web/state.rs` - queue_status コメント更新
  - `dashboard/src/api/types.ts` - ChangeStatus に 'rejected' 追加

## Acceptance Criteria

1. Acceptance が Blocked を返した場合、ワークスペースが error 状態で残らず、rejection フローが最後まで完了する
2. `openspec/changes/<change_id>/REJECTED.md` が base ブランチにコミットされる
3. `openspec resolve <change_id>` が呼ばれ、change が resolved 状態になる
4. Worktree が削除される
5. TUI / Web で `"rejected"` ステータスが表示され、error とは異なる色になる
6. `cflx run` 再実行時に rejected 済み change がキューに入らない
7. TUI での手動 Space キュー追加でも rejected change は NoOp になる
