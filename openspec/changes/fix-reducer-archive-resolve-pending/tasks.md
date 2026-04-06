## Implementation Tasks

- [ ] `src/orchestration/state.rs` の `ExecutionEvent::ChangeArchived` (parallel mode) を修正し、同一 project 内の他 change に `ActivityState::Resolving` がある場合は `WaitState::ResolveWait`、それ以外は `WaitState::MergeWait` にする (verification: reducer 実装で archived change の遷移先が project-scoped active resolve に応じて分岐する)
- [ ] reducer 単体テストを追加し、別 change が resolving 中に archive 完了した change が `resolve pending` になることを検証する (verification: `src/orchestration/state.rs` のテストで `display_status(...) == "resolve pending"` を確認)
- [ ] reducer 単体テストを追加または更新し、active resolve がない場合は従来どおり `merge wait` になることを検証する (verification: `src/orchestration/state.rs` のテストで `display_status(...) == "merge wait"` を確認)
- [ ] `python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate fix-reducer-archive-resolve-pending --strict` を実行して proposal を検証する (verification: validation passed)

## Future Work

- reducer の post-archive 判定と TUI orchestrator 側の post-archive dispatch の責務分離を再整理する
- server mode / parallel bridge を含む end-to-end テストで project-scoped resolve queue の挙動を追加検証する
