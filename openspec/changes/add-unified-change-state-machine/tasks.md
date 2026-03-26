## Phase 1: reducer の土台を追加する

- [ ] 1.1 `src/orchestration/state.rs` に `RunLifecycle`, `QueueIntent`, `ActivityState`, `WaitState`, `TerminalState`, `WorkspaceObservation`, `ChangeRuntimeState` を追加する (verification: `cargo build`)
- [ ] 1.2 `OrchestratorState` に `change_runtime: HashMap<String, ChangeRuntimeState>` と reducer-owned resolve wait queue を追加し、`new()` で初期 change を `NotQueued + Idle + None + None` 相当で初期化する (verification: `cargo test test_orchestrator_state_initializes_change_runtime`)
- [ ] 1.3 `ChangeRuntimeState` の不変条件ヘルパーを追加し、`Merged` と active activity の同時成立、`ResolveWait` と `Resolving` の同時成立などの禁止組み合わせを判定できるようにする (verification: `cargo test test_change_runtime_invariants`)
- [ ] 1.4 `display_status(change_id)` を追加し、既存 UI 語彙 `not queued`, `queued`, `blocked`, `applying`, `accepting`, `archiving`, `merge wait`, `resolve pending`, `archived`, `merged`, `error` を reducer state から導出できるようにする (verification: `cargo test test_display_status_derivation`)
- [ ] 1.5 `is_active_status(change_id)` など、並列スロット判定で必要な active/inactive 判定ヘルパーを shared state 側に追加する (verification: `cargo test test_runtime_state_active_classification`)

## Phase 2: reducer API を定義する

- [ ] 2.1 `src/orchestration/state.rs` に `ReducerCommand`, `ReduceOutcome`, `ReducerEffect` を追加する (verification: `cargo build`)
- [ ] 2.2 `OrchestratorState::apply_command()` を実装し、少なくとも `AddToQueue`, `RemoveFromQueue`, `ResolveMerge`, `StopChange` を処理できるようにする (verification: `cargo test test_apply_command_queue_intent`)
- [ ] 2.3 `OrchestratorState::apply_execution_event()` を reducer-owned runtime state 更新に拡張し、`ApplyStarted`, `AcceptanceStarted`, `ArchiveStarted`, `ChangeArchived`, `MergeDeferred`, `ResolveStarted`, `ResolveCompleted`, `MergeCompleted`, `ProcessingError`, `ApplyFailed`, `ArchiveFailed`, `DependencyBlocked`, `DependencyResolved`, `ChangeStopped` を扱う (verification: `cargo test test_apply_execution_event_transitions`)
- [ ] 2.4 `OrchestratorState::apply_observation()` を実装し、workspace 観測から `MergeWait` の復元・解除を行う。ただし active activity は上書きしない (verification: `cargo test test_apply_observation_reconcile_merge_wait`)
- [ ] 2.5 duplicate / stale / late input を no-op として扱う idempotency テストを追加する（例: `Merged` 後の `ResolveFailed`、重複 `ApplyStarted`）(verification: `cargo test test_reducer_idempotency_and_precedence`)

## Phase 3: TUI command 経路を reducer に通す

- [ ] 3.1 `src/tui/command_handlers.rs` の `TuiCommand::AddToQueue` / `RemoveFromQueue` 処理で、`DynamicQueue` 更新前に `shared_state.write().apply_command(...)` を呼ぶようにする (verification: `cargo test test_add_to_queue_command_updates_reducer_before_dynamic_queue`)
- [ ] 3.2 `src/tui/command_handlers.rs` の `ResolveMerge` / `StopChange` 系も reducer command を先に適用するようにする (verification: `cargo test test_resolve_merge_command_updates_reducer_wait_queue`)
- [ ] 3.3 `src/tui/state.rs` の `toggle_selection()` / `handle_toggle_running_mode()` から `queue_status = ...` の直接代入を外し、command 発行責務だけに縮退させる (verification: `cargo test test_running_mode_toggle_emits_commands_without_local_status_mutation`)
- [ ] 3.4 `MergeWait` / `ResolveWait` 行の Space と `M` の挙動が reducer state と一致する回帰テストを追加する (verification: `cargo test test_merge_wait_queue_operations` + `cargo test test_resolve_wait_queue_operations`)

## Phase 4: orchestrator / parallel event 経路を reducer に通す

- [ ] 4.1 `src/tui/orchestrator.rs` の serial 実行で送信している execution events が、shared state の `apply_execution_event()` に全て反映されることを確認し、不足イベントを追加する (verification: `cargo test test_serial_orchestrator_updates_reducer_for_full_lifecycle`)
- [ ] 4.2 `src/tui/orchestrator.rs` の parallel forward task が、転送する全 event を reducer に反映することを確認し、不足があれば追加する (verification: `cargo test test_parallel_forward_task_updates_reducer_for_full_lifecycle`)
- [ ] 4.3 `src/parallel/` の merge / resolve 系イベント (`MergeDeferred`, `ResolveStarted`, `ResolveCompleted`, `MergeCompleted`, `ResolveFailed`) が reducer の precedence 規則で処理されることを確認する (verification: `cargo test test_parallel_merge_events_drive_reducer_wait_states`)
- [ ] 4.4 stop 後の遅延イベントが terminal / stopped 状態を退行させないテストを追加する (verification: `cargo test test_late_events_after_stop_do_not_regress_state`)

## Phase 5: refresh と workspace recovery を reconcile 経路へ移す

- [ ] 5.1 `src/tui/runner.rs` の worktree refresh で生成している `merge_wait_ids` / `worktree_not_ahead_ids` などの UI 状態上書き用データを、`WorkspaceObservation` 構築に置き換える (verification: `cargo build`)
- [ ] 5.2 `src/tui/state.rs` の `handle_changes_refreshed()` から `apply_merge_wait_status()` / `auto_clear_merge_wait()` 依存を外し、shared state の observation reconcile を読む構成に変える (verification: `cargo test test_changes_refreshed_uses_reducer_observation_path`)
- [ ] 5.3 `src/tui/state.rs` の `apply_merge_wait_status()` と `auto_clear_merge_wait()` を削除し、同等の挙動を `apply_observation()` でカバーする (verification: `cargo test test_merge_wait_release_after_external_merge`)
- [ ] 5.4 `WorkspaceState::Archived` の refresh 復元先が `ResolveWait` ではなく `MergeWait` であることをテストと実装で固定する (verification: `cargo test test_workspace_archived_recovers_merge_wait`)
- [ ] 5.5 queue 済み change が、別 change の `MergeWait` や refresh observation により上書きされない回帰テストを追加する (verification: `cargo test test_queue_add_not_overwritten_by_merge_wait_refresh`)

## Phase 6: TUI / Web の表示読取を shared state に寄せる

- [ ] 6.1 `src/tui/state.rs` と `src/tui/render.rs` で、表示・キーヒント・active 判定に `display_status(change_id)` と active 判定ヘルパーを使うようにする (verification: `cargo test test_tui_uses_reducer_display_status`)
- [ ] 6.2 `ChangeState.queue_status` を transitional field として残す場合は読み取り専用にし、最終的に不要であれば削除する (verification: `cargo build`)
- [ ] 6.3 `src/web/state.rs` で既存 API shape を変えず、shared state から `queue_status` 文字列を導出する adapter を使うようにする (verification: `cargo test test_web_snapshot_uses_reducer_display_status_without_payload_change`)
- [ ] 6.4 dependency block / unblock / merge wait / resolving の組み合わせで、TUI と Web の表示語彙が一致するテストを追加する (verification: `cargo test test_display_status_consistency_between_tui_and_web`)

## Phase 7: 仕上げと全体検証

- [ ] 7.1 `tui-architecture`, `parallel-execution`, `orchestration-state` の spec scenario に対応する unit/integration test 名をソースにコメントまたは命名で対応付ける (verification: 関連テストが `cargo test` で PASS)
- [ ] 7.2 reducer state と既存集計 (`pending_changes`, `archived_changes`, `current_change_id`, `apply_count`) の整合性確認テストを追加する (verification: `cargo test test_reducer_runtime_and_legacy_aggregates_stay_consistent`)
- [ ] 7.3 `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` を通す (verification: 各コマンド成功)

## Future Work

- remote / server mode でも reducer-derived display status を使う
- reducer state の遷移履歴を observability 用に記録する
- reducer state の永続化が必要かを別 proposal で検討する
