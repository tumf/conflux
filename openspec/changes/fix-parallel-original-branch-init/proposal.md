---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/vcs/git/mod.rs
  - src/parallel/orchestration.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-merge/spec.md
---

# Change: parallel merge の original branch 初期化抜けを修正

**Change Type**: implementation

## Problem / Context

parallel 実行で archived change を base branch へ merge する経路において、`src/parallel/merge.rs` が `workspace_manager.original_branch()` を直接参照している。base branch 名が未初期化のままこの経路に入ると `Original branch not initialized` で即失敗し、archived change が merge 完了できない。

この失敗は Git 競合や base dirty と同じ `MergeWait` っぽい停滞に見えるが、実際には内部状態初期化の不備であり、ユーザー介入を必要としない。既存仕様では `MergeWait` は手動介入が必要な deferred merge の待機状態として扱われ、archived change は merge 完了または resolve/deferred 判定に進む前提であるため、この初期化漏れは仕様と運用期待の両方に反する。

## Proposed Solution

- parallel merge / deferred-merge / archive-complete handoff の前に、Git backend の original/base branch を必ず初期化する
- base branch 未初期化が発生した場合は、内部で recover 可能なら self-heal し、recover 不能な場合のみ実行エラーとして扱う
- archived change が base branch 初期化漏れだけで `MergeWait` に滞留しないよう、状態遷移とイベントの期待値を明確化する
- dependency 判定や resume 判定など `original_branch()` 依存箇所でも、手動介入不要な初期化漏れを dependency unresolved / merge wait と誤分類しないことを明文化する

## Acceptance Criteria

- archived change の merge 開始時に base branch 名が未初期化でも、Git backend が現在の base branch を取得して merge を継続できる
- recover 不能な detached HEAD 等を除き、`Original branch not initialized` は user-facing merge failure として露出しない
- base branch 初期化漏れのみが原因のケースでは、change は `MergeWait` に留まらず merge handling を継続する
- recover 不能なケースは `MergeWait` ではなく明示的なエラーとして扱われ、手動 cleanup 待ちの deferred merge と区別される
- parallel resume / dependency resolution の original branch 参照も同じ初期化保証を共有する

## Out of Scope

- 実際の merge conflict 解消アルゴリズム変更
- base dirty / manual resolve / deferred merge policy の変更
- Git 以外の VCS backend 追加
