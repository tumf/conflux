---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/dispatch.rs
  - src/serial_run_service.rs
  - src/task_parser.rs
  - src/orchestration/acceptance.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/cli/spec.md
---

# Change: acceptance follow-up 記録失敗を terminal error にしない

**Change Type**: implementation

## Premise / Context

- `.last-checked` 以降の cflx 実行ログでは `classify-acceptance-followup-routing` の acceptance が妥当な `FAIL` を返した直後、`Failed to record acceptance follow-up tasks ... tasks.md: No such file or directory` により change 全体が error 終了していた。
- 現行 runtime は acceptance fail のたびに `src/parallel/dispatch.rs` と `src/serial_run_service.rs` から `task_parser::record_acceptance_follow_up(...)` を呼び、workspace 内 `openspec/changes/<change_id>/tasks.md` を決め打ち更新している。
- しかし `record_acceptance_follow_up` は active-path 固定の `read_to_string(tasks_path)` で開始し、tasks file 不在時の archive fallback や warning degrade を持たない。
- 既存 active change `classify-acceptance-followup-routing` は follow-up 内容の分類を扱い、archived rejecting recovery fallback は archived proposal `resume-rejecting-from-archived-worktree` が扱っていたが、acceptance fail 記録失敗そのものを secondary error として扱う契約は未定義である。

## Requested Artifact

- implementation proposal for making acceptance-fail follow-up recording path-aware and non-fatal
- canonical requirement that acceptance verdict failure remains primary even if follow-up persistence degrades
- regression coverage for missing active tasks.md, archive fallback, and warning-only persistence failure

## Problem / Context

Conflux は acceptance が `FAIL` を返した時、その findings を tasks.md に追記しようとして失敗すると、acceptance fail 自体よりも tasks persistence failure を優先して change 全体を terminal `Error` にしてしまう。これにより、repo 修正で解決すべき普通の acceptance fail が、metadata path 不整合だけで実行系 error に増幅される。

この振る舞いは二重に問題がある。第一に、acceptance verdict の主結果と付随的な follow-up persistence failure が分離されていない。第二に、書き込み先解決が active path 固定で、archive 位置や別 canonical tasks location への fallback を持たない。結果として「acceptance fail は正当なのに runtime error になる」揺らぎが再発しうる。

## Proposed Solution

acceptance fail の主結果を維持したまま、follow-up 記録を path-aware かつ degradation-tolerant にする。

- acceptance fail 後の follow-up persistence は active tasks path だけでなく、必要に応じて archive tasks location を探索できる canonical resolver を使う。
- active/archived いずれの tasks file も見つからない場合でも、runtime は acceptance verdict `FAIL` を primary result として保持し、persistence failure は warning / supplemental context として記録する。
- `task_parser::record_acceptance_follow_up` またはその呼び出し側の責務を見直し、`tasks.md is now updated by the acceptance agent itself` というコメントと実際の runtime update 挙動の不一致を解消する。
- logs / history / error wording は `acceptance failed` と `follow-up persistence degraded` を区別し、secondary persistence issue が primary diagnosis を上書きしないようにする。
- regression tests を追加し、missing active tasks path・archived tasks fallback・no tasks path at all の各ケースで、acceptance fail が terminal error に増幅されないことを固定する。

## Acceptance Criteria

- acceptance が `FAIL` を返したケースでは、active tasks path が存在しなくても change 全体が即 terminal `Error` にならない。
- archived tasks location が存在する場合、follow-up persistence は active path 不在でも archived tasks file へ記録できる。
- active/archived どちらの tasks file も存在しない場合でも、runtime は acceptance fail を primary outcome として保持し、follow-up persistence failure は warning または supplemental context として観測できる。
- user-visible logs / events / tests は `acceptance verdict failed` と `follow-up persistence degraded` を区別して確認できる。
- runtime / prompt / code comments の責務説明は、「accept agent が更新するのか」「runtime が追記するのか」のどちらが canonical か分かる形に整理される。

## Explicit Completion Conditions

- OpenSpec delta が acceptance fail primary outcome と follow-up persistence degradation の分離を canonical spec として記述している。
- `src/parallel/dispatch.rs` と `src/serial_run_service.rs` の fail path が tasks file 不在だけで terminal error を返さない。
- `src/task_parser.rs` または関連 helper に active/archive fallback もしくは同等の canonical path resolution が追加される。
- Rust tests が active path missing、archive fallback、no tasks path の各ケースをカバーする。
- `cflx openspec validate degrade-acceptance-followup-persistence --strict --evidence warn` が成功する。

## Out of Scope

- acceptance follow-up の remediation vs blocker-only 分類そのもの
- acceptance verdict vocabulary (`blocked` / `gated`) の統一
- rejecting review 専用 recovery path の別件改善
