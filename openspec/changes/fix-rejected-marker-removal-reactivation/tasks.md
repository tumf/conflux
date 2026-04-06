## Implementation Tasks

- [ ] 1. `ChangesRefreshed` ハンドラで reactivation を処理する: `src/orchestration/state.rs` の `ChangesRefreshed` reconcile 時、active change list に含まれている change が `TerminalState::Rejected` を持つ場合、terminal / activity / wait_state / queue_intent をデフォルトに戻す (verification: reducer test で rejected → ChangesRefreshed with change re-appearing → display_status が "not queued" に戻ることを確認)
- [ ] 2. TUI の `apply_display_statuses_from_reducer` が reactivated change を正しく反映することを確認する: `src/tui/state.rs` の display status sync が rejected → not queued の遷移を suppress しないことを確認し、必要なら guard 条件を追加する (verification: TUI state test で rejected が not queued に戻ることを検証)
- [ ] 3. `AddToQueue` が reactivated change に対して受理されることを確認する: `src/orchestration/state.rs` の既存テスト `test_apply_command_queue_intent` に rejected → reactivation → AddToQueue → queued の遷移パスを追加する (verification: テストが pass する)
- [ ] 4. Web state の reactivation 収束を確認する: `src/web/state.rs` の `ChangesRefreshed` 処理が rejected change の再表出で正しく `rejected` 以外を表示することをテストで確認する (verification: テストが pass する)
- [ ] 5. spec delta を canonical spec の既存 Requirement に追加する: `Rejected Change Exclusion from Change Listing` requirement に reactivation scenario を追加する (verification: `cflx.py validate --strict` が pass する)

## Future Work

- file watcher による `REJECTED.md` 削除の即時反映
- rejected marker の長期保管 / archive ツリー移行方針
