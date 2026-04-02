## Implementation Tasks

- [x] 1. `src/merge_stall_monitor.rs` を削除する (verification: ファイルが存在しないこと)
- [x] 2. `src/lib.rs` と `src/main.rs` から `mod merge_stall_monitor;` を削除する (verification: `rg 'merge_stall_monitor' src/lib.rs src/main.rs` が 0 件)
- [x] 3. `src/parallel/orchestration.rs` から monitor 関連コードを削除する: `use crate::merge_stall_monitor::MergeStallMonitor`, `monitor_stop_token`, `merge_stall_monitor_handle` の定義・起動・停止をすべて除去 (verification: `rg 'merge_stall|MergeStallMonitor|monitor_stop_token' src/parallel/orchestration.rs` が 0 件)
- [x] 4. `src/config/types.rs` から `MergeStallDetectionConfig` 構造体、`default_merge_stall_detection_enabled` 関数、`OrchestratorConfig::merge_stall_detection` フィールド、`get_merge_stall_detection` メソッド、merge 関連コードを削除する (verification: `rg 'MergeStallDetection|merge_stall_detection' src/config/types.rs` が 0 件)
- [x] 5. `src/config/mod.rs` から merge stall 関連テスト (`test_merge_stall_detection_defaults`, `test_parse_merge_stall_detection_config`, `test_merge_stall_detection_disabled`) を削除する (verification: `rg 'merge_stall' src/config/mod.rs` が 0 件)
- [x] 6. `src/config/defaults.rs` から merge stall 関連のデフォルト定数を削除する (verification: `rg 'MERGE_STALL' src/config/defaults.rs` が 0 件)
- [x] 7. `cargo fmt --check && cargo clippy -- -D warnings && cargo test` が全て pass する (verification: CI 相当のチェックが通ること)

## Future Work

- queue / scheduler の実進捗に基づく health monitor が必要になったら別 proposal で設計する
