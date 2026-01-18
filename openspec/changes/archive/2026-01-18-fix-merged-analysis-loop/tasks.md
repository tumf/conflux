## 1. Implementation
- [x] 1.1 queuedのみをanalysis対象にするフィルタ条件を追加する
- [x] 1.2 queued外のchangeをanalysis対象から除外する
- [x] 1.3 実行中changeがなくqueuedも空のときに終了する判定を追加する
- [x] 1.4 queueが空のときはanalysisを実行しない

## 2. Tests
- [x] 2.1 queuedのみがanalysis対象になることを検証する
- [x] 2.2 queued外のchangeがanalysis対象から除外されることを検証する
- [x] 2.3 実行中・queuedが空のときに並列実行が終了することを検証する

## 3. Validation
- [x] 3.1 cargo fmt
- [x] 3.2 cargo clippy
- [x] 3.3 cargo test


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) Task 1.1 marked complete but implementation only added to deprecated `execute_with_reanalysis` method (src/parallel/mod.rs:661)
  2) Production code uses `execute_with_order_based_reanalysis` which does NOT have the queued-only filter
  3) CLI call path: src/orchestrator.rs:983 `run_parallel` → src/parallel_run_service.rs:184 `execute_with_order_based_reanalysis` (no filter)
  4) TUI call path: src/tui/orchestrator.rs:1421 `run_parallel_with_channel_and_queue_state` → src/parallel_run_service.rs:273 `execute_with_order_based_reanalysis` (no filter)
  5) The queued-only filter exists only in src/parallel/mod.rs:661-749 within `execute_with_reanalysis`, which has `#[deprecated]` attribute (line 415)
  6) Dead code: The entire queued-only filter implementation is unreachable from CLI/TUI execution paths
  7) Integration check FAILED: Feature is not executed in real flow despite all tasks marked [x]
- [x] 4.1 Add queued-only filter to `execute_with_order_based_reanalysis` method
- [x] 4.2 Run cargo fmt
- [x] 4.3 Run cargo clippy
- [x] 4.4 Run cargo test
