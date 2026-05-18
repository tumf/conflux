---
change_type: implementation
priority: high
dependencies: []
references:
  - src/orchestration/state.rs
  - openspec/specs/code-maintenance/spec.md
  - openspec/CONSTITUTION.md
---

# Orchestration state reducer の責務分割

**Change Type**: implementation

## Problem/Context

`src/orchestration/state.rs` は tracked file の中でも最大級の 4,586 行で、状態型、reducer、wait queue 操作、観測同期、単体テストが同一ファイルに集中している。特に `apply_command` と `apply_execution_event` が terminal/activity/wait state と resolve/reject wait queue を同時に更新しており、既存仕様が要求する「リファクタリング安全性の担保」を満たすには、先に状態遷移を固定してから責務を分ける必要がある。

候補ランキングでは、(1) 中心性が高い、(2) 行数が大きい、(3) 状態遷移の退行リスクが高い、(4) 既存 `code-maintenance` 仕様が reducer の characterization を明示している、という理由で最上位に選定した。

### Evidence

- `src/orchestration/state.rs:59` 以降で状態型、reducer、queue 操作、テストが同居している。
- `src/orchestration/state.rs:1140` の `apply_command` がユーザー操作と queue/wait/terminal 更新を直接扱っている。
- `src/orchestration/state.rs:1294` の `apply_execution_event` が実行イベントごとの副作用を大きな match に集約している。
- `src/orchestration/state.rs:4308` 以降には resolve/merge/retry 系の回帰テストが多数あり、状態遷移の複雑さが高い。

## Proposed Solution

- 振る舞いを変えず、`OrchestratorState` の公開 API と表示ステータスを維持したまま reducer 内部を責務別ヘルパーまたはサブモジュールへ分割する。
- `ReducerCommand` と `ExecutionEvent` の代表的な状態遷移を characterization test で先に固定する。
- resolve/reject wait queue の追加・削除・昇格条件を専用ヘルパーへ集約し、terminal state と queue intent の更新箇所を追跡しやすくする。
- `openspec/CONSTITUTION.md` に従い、状態判断は workspace/git/base-branch 由来の既存入力に限定し、外部ログや UI 状態を workflow-control 入力にしない。

## Acceptance Criteria

- `OrchestratorState` の既存公開 API、`display_status`、`ReduceOutcome`、terminal/activity/wait/queue intent の外部挙動が変わらない。
- `ReducerCommand` と主要 `ExecutionEvent` の characterization test がリファクタ前後で同じ結果を示す。
- resolve/reject wait queue の重複防止、削除、昇格順序が既存と同等である。
- リファクタ後も workspace-local workflow state の憲法制約に違反しない。
- `cargo fmt --check` と対象 reducer テストが成功する。

## Explicit Completion Conditions

- `src/orchestration/state.rs` の reducer 本体が、コマンド処理、実行イベント処理、wait queue 操作、テストのいずれかで明確に責務分割されている。
- 分割後のコードから見ても、成功、失敗、stalled、resolve/reject wait、dequeue/retry の各状態遷移の入口が追跡できる。
- characterization test が、既存仕様 `code-maintenance` の reducer command / execution event scenario を満たしている。
- 既存 CLI/TUI/Web の状態表示 contract を変更していないことがテストまたは明示的な差分確認で示されている。

## Out of Scope

- 新しい workflow 状態、永続状態、UI 表示、CLI/API contract の追加。
- serial/parallel mode の挙動変更。
- 既存の状態名、表示文字列、イベント型の変更。
