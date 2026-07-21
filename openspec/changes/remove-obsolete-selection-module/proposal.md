---
change_type: implementation
priority: low
dependencies: []
references:
  - src/orchestration/selection.rs
  - src/orchestration/mod.rs
  - src/serial_run_service.rs
  - src/orchestrator.rs
  - openspec/specs/code-maintenance/spec.md
verifications:
  - id: obsolete-selection-removal-checks
    requirement: Removing the unreachable selection module preserves the active serial and parallel selection paths
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: serial selection unit tests and successful all-feature compilation after module removal
    rerun: cargo test serial_run_service::tests --lib && cargo check --all-features
    prerequisites: []
---

# 未使用のorchestration selection moduleを削除する

**Change Type**: implementation

## Problem/Context

`src/orchestration/selection.rs:6-10` は、selection logicが `SerialRunService` へ移行済みで現在は未使用であることを明記し、module全体を `#![allow(dead_code)]` で残している。このmoduleの関数は同ファイル内のtest以外から参照されず、現役のserial選択は `src/orchestrator.rs:1080-1091` から `SerialRunService::select_next_change` を使用する。

旧moduleはcomplete change優先、現役serviceはincomplete change優先という異なる方針も含むため、参照用コードとして残すほど将来の修正先を誤認しやすい。Git履歴で復元可能な到達不能コードを削除し、現役selection ownerを明確にする。

## Proposed Solution

- `src/orchestration/selection.rs` とそのmodule登録を削除する。
- `src/orchestration/mod.rs` の説明から、削除したselection責務と進行中統合を示す古い記述を除く。
- 現役の `SerialRunService` およびparallel analyzer/ordering実装は変更しない。
- 削除対象だけに属するtestは削除し、現役selectionの既存testを回帰証拠として実行する。

## Acceptance Criteria

- `src/orchestration/selection.rs` と `orchestration::selection` のmodule宣言・参照が存在しない。
- serial modeは引き続き `SerialRunService::select_next_change` を使用する。
- parallel modeのanalyzerとorder-based selectionは変更されない。
- CLI/TUIのchange選択順、エラー、ログ、promptは変更されない。
- dead-code suppressionを削除してもall-feature buildが成功する。

## Explicit Completion Conditions

- `src/orchestration/selection.rs` が削除され、`src/orchestration/mod.rs` が現行責務だけを記述している。
- repository-wide searchでproductionまたはtestから `orchestration::selection` 参照が検出されない。
- `src/serial_run_service.rs` のselection unit testsが既存期待値で成功する。
- `cargo test serial_run_service::tests --lib`、`cargo check --all-features`、`cargo fmt --all -- --check` が成功する。

## Out of Scope

- `SerialRunService`、serial mode、parallel analyzerの削除または再設計。
- change選択優先順位、dependency判断、LLM analysis promptの変更。
- 他の `#[allow(dead_code)]` 箇所の一括整理。
