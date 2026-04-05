## Specification Tasks

- [ ] Phase 1a: `derive-reanalysis-from-scheduler-state` を実行する — parallel-execution spec に state-driven reanalysis scheduling 要件が追加される (verification: proposal が archive 済み)
- [ ] Phase 1b: `define-rejecting-resume-state` を実行する — orchestration-state spec に RejectionReviewCompleted 遷移が追加される (verification: proposal が archive 済み)
- [ ] Phase 2: `target-workspace-status-events` を実行する — orchestration-events spec に change 単位ターゲティング要件が追加される (verification: proposal が archive 済み、Phase 1 完了後)
- [ ] Phase 3: `align-reducer-derived-status` を実行する — server-api spec に WebSocket が Reducer 正典から表示ステータスを導出する要件が追加される (verification: proposal が archive 済み、Phase 2 完了後)
- [ ] 全 4 proposal archive 後に、TUI/WebSocket/Dashboard の表示一致を回帰テストで確認する (verification: `cargo test --lib server` + `cargo test --lib orchestration`)

## Future Work

- Serial モードの Rejecting/Resolving サポート定義
- `WorkspaceStatus` enum の段階的廃止検討
- `WorkspaceState` の Accepting バリアント追加または Applied 中間ステータス整理
