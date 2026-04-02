---
change_type: implementation
priority: high
dependencies: []
references:
  - src/merge_stall_monitor.rs
  - src/parallel/orchestration.rs
  - src/config/types.rs
  - src/config/mod.rs
  - src/lib.rs
  - src/main.rs
---

# Change: Remove merge stall monitor

**Change Type**: implementation

## Why

merge stall monitor は base branch の最後の merge commit 時刻を見て stall を判定するが、これは queue / scheduler の実行進捗を反映していない。stop 権限を持たせると queue を壊し（実際に壊した）、stop 権限を外すと継続実行において実質的な価値がない。監視対象も監視目的もズレており、現状では有害または無用である。

## What Changes

- `src/merge_stall_monitor.rs` を削除
- `src/parallel/orchestration.rs` から monitor の起動・停止コードを削除
- `src/config/types.rs` から `MergeStallDetectionConfig` と関連フィールドを削除
- `src/config/mod.rs` から関連テストを削除
- `src/lib.rs`, `src/main.rs` から `mod merge_stall_monitor` を削除
- `.cflx.jsonc` の `merge_stall_detection` 設定は無視される（未知キーとして無害）

## Impact

- Affected specs: `parallel-execution`
- Affected code: `src/merge_stall_monitor.rs`, `src/parallel/orchestration.rs`, `src/config/types.rs`, `src/config/mod.rs`, `src/lib.rs`, `src/main.rs`

## Out of Scope

- queue / scheduler の実進捗に基づく新しい health monitor の設計（将来別提案）
