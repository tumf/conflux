---
change_type: implementation
priority: high
dependencies: []
references:
  - src/tui/state.rs
  - src/web/state.rs
  - openspec/specs/tui-resolve/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# Change: resolving 中に他 change の apply/accept/archive がブロックされるバグを修正

**Change Type**: implementation

## Why

アーキテクチャ上の関係は `Orchestration 1--* Project 1--* Change` であり、`Resolving` は個別 Change の `ActivityState`（Change レベルのステータス）である。しかし TUI の `is_resolving: bool` フラグはこれを Project レベルのロックとして誤用しており、`start_processing`、`resume_processing`、`retry_error_changes` を Project 内の全 Change に対してブロックしている。結果として、ある Change が resolving 中だと同一 Project 内の他の Change の apply/accept/archive パイプラインが全停止する。

resolving が同一 Project 内の他の Change に影響を与えるべきタイミングは以下の2つだけ:
1. archived → merge wait 遷移時（resolve キューへの追加判定）
2. resolve 完了時（次の pending change が resolving に遷移）

## What Changes

- `start_processing()` から `is_resolving` ガードを削除
- `resume_processing()` から `is_resolving` ガードを削除
- `retry_error_changes()` から `is_resolving` ガードを削除
- `request_merge()` (M キー) の `is_resolving` ガードは維持（resolve 同士の直列化に必要）
- `web/state.rs` の `MergeDeferred` イベントハンドラは変更なし（resolve のキュー追加ロジックは正しい）

## Impact

- Affected specs: `tui-state`
- Affected code: `src/tui/state.rs`, テスト3件（`test_start_processing_blocked_while_resolving` 等）
