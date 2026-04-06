---
change_type: implementation
priority: high
dependencies: []
references:
  - src/analyzer.rs
  - src/parallel/queue_state.rs
  - src/orchestration/state.rs
  - openspec/specs/parallel-analysis/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# Change: Restore blocked status when analyzer dependencies are unresolved

**Change Type**: implementation

## Premise / Context

- ユーザー報告では、analyzer は正しい dependency を返しているのに dependent change が `blocked` にならない。
- この挙動は regression であり、以前は依存未解決 change が `blocked` として扱われていた前提で修正する。
- Conflux の parallel 実行では analyzer の `dependencies` が hard dependency source であり、frontmatter は analyzer の参考情報に過ぎない。
- `src/parallel/queue_state.rs` の dispatch 選定は analyzer 結果を使って blocked / queued / dispatch を分岐し、`src/orchestration/state.rs` は `DependencyBlocked` を reducer-owned runtime state に反映して display status `blocked` を導出する。

## Problem / Context

現在の parallel scheduler では、analyzer が `analysis_result.dependencies` に未解決 dependency を返していても、後続 change が `DependencyBlocked` として reducer に反映されず、UI / Web / runtime display status が `blocked` に遷移しないケースがある。

この不整合により、実行不能な dependent change が単なる `queued` あるいは未評価状態のまま見え、依存待ちで停止していることが利用者に伝わらない。これは以前成立していた blocked 表示の回帰であり、dependency-aware scheduling の観測可能な契約を破っている。

## Proposed Solution

parallel 実行の scheduler は、analyzer が返した hard dependency を dispatch 可否判定だけでなく blocked state 反映にも一貫して用いるよう修正する。

- analyzer の `dependencies` に未解決 dependency がある change は、dispatch 可否とは独立に必ず `DependencyBlocked` として扱う
- blocked 判定は available slot・order 走査・再分析トリガの違いに依存せず、未解決 dependency の存在だけで決まるようにする
- dependency が解決済みに遷移した change は再分析時に `DependencyResolved` で reducer を更新し、`blocked` から通常の queue 状態へ戻す
- reducer / TUI / Web は scheduler から送られる dependency block / resolve イベントに基づき同じ display status に収束する
- regression を固定するため、analyzer が正しい dependency graph を返したときの blocked 表示を unit test でカバーする

## Acceptance Criteria

- analyzer が `change-b -> change-a` の dependency を返し、`change-a` が base branch に未 merge のとき、`change-b` は `DependencyBlocked` として reducer に反映される
- 上記ケースで `change-b` の display status は `blocked` になる
- blocked 判定は available slot 数や order 上の位置にかかわらず適用される
- dependency が解決した再分析では `DependencyResolved` が反映され、`change-b` は `blocked` から通常の queue 状態へ戻る
- TUI / Web / shared orchestration state は dependency block / resolve の結果について同じ状態に収束する
- regression test により、analyzer が正しい dependency を返しているのに blocked にならないケースが再発しない

## Out of Scope

- analyzer の dependency 推論ロジック自体の見直し
- frontmatter を hard dependency source に格上げする仕様変更
- dependency とは無関係な merge wait / resolve wait / rejected flow の再設計
