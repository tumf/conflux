---
change_type: implementation
priority: high
dependencies:
  - align-archive-readiness-failure-reporting
references:
  - src/parallel/executor.rs
  - src/parallel/dispatch.rs
  - src/execution/state.rs
  - src/orchestration/archive.rs
  - src/agent/history_ops.rs
  - src/history.rs
  - src/events.rs
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-events/spec.md
  - openspec/changes/align-archive-readiness-failure-reporting/proposal.md
---

# Change: archive retry / resume 理由を durable に保持して再ループ理由を可視化する

**Change Type**: implementation

## Premise / Context

- 現行 archive ループは同一プロセス内では `ArchiveHistory` を prompt に注入し、verification failure reason を次回 retry に渡している。
- しかし archive failure / retry reason は durable state として保存されず、resume や再起動をまたぐと失敗文脈が消える。
- 並列 resume routing は `Archived` workspace の merge handoff を正しく扱うが、`Archiving` または archive retry が継続する理由は file state と汎用 log に埋もれやすい。
- 既存 active change `align-archive-readiness-failure-reporting` は archive failure の root cause surfacing を扱うが、resume 境界をまたぐ persistence と structured retry reason event までは扱っていない。

## Requested Artifact

- implementation proposal for durable archive retry/resume reason persistence
- user-visible observability for why archive re-loop / retry / resume happened

## Problem / Context

Conflux の archive フローは、同一ラン内では retry reason を prompt に引き継げる一方、resume/再起動境界ではその文脈を durable に保持していない。結果として、`Archiving` workspace や verification-failed archive を再開した際に、runtime は「なぜ再び archive に入ったのか」「前回どこで失敗したのか」を十分に説明できない。

現状の並列 archive 実装は verification failure 時に `Archive verification failed ... retrying archive command` のような汎用ログを出すが、change directory 残留・archive prerequisite blocker・post-archive commit failure などの差分を構造化して downstream event / UI / resume routing に渡していない。そのためユーザー視点では「また同じ loop に入った」「前回の失敗が引き継がれていない」ように見える。

## Proposed Solution

archive retry / resume の理由を durable state と structured event reason として扱い、同一ラン内・resume 後・UI 表示のいずれでも同じ説明が見えるようにする。

- archive attempt ごとの primary failure reason を enum 化し、verification failure・prerequisite blocker・command failure・post-archive completion failure・stall を区別して保持する。
- worktree 外 durable state として archive resume state を保存し、resume 時に `Archiving` / `Applied` / `Archived` 判定と合わせて「前回 archive がどこで止まったか」を参照できるようにする。
- archive retry / resume / terminal handoff のイベントに structured reason を含め、TUI / Web / log が「retrying archive command」だけでなく root cause を表示できるようにする。
- archive prompt history には既存の `ArchiveHistory` を維持しつつ、resume 後の最初の archive retry でも durable state 由来の直前失敗理由を復元して agent に渡せるようにする。
- 既存 active change `align-archive-readiness-failure-reporting` が導入する root-cause surfacing と整合し、この proposal はそれを resume persistence / observability 層へ拡張する。

## Acceptance Criteria

- archive verification failure, prerequisite blocker, command failure, stall, post-archive completion failure が runtime 上で区別され、少なくとも logs / events / retry context のいずれかから同じ primary reason を参照できる。
- `Archiving` workspace を resume したとき、runtime は file state だけでなく durable archive resume state に基づいて「前回 archive がどの理由で継続扱いになったか」を復元できる。
- resume/再起動後の最初の archive retry でも、agent prompt または equivalent runtime context に直前 archive failure reason が含まれる。
- `Archived` workspace の merge handoff は引き続き terminal 扱いのままであり、durable archive state の追加によって apply/acceptance/archive pipeline に再突入しない。
- user-visible logs / events / tests は、archive が再ループした理由を generic な retry message だけでなく reason-aware に観測できる。

## Explicit Completion Conditions

- OpenSpec delta が archive retry reason persistence、resume observability、terminal archived handoff invariants を canonical spec として記述している。
- `src/parallel/executor.rs`, `src/parallel/dispatch.rs`, `src/orchestration/archive.rs`, `src/history.rs` または同等の責務箇所に、archive primary reason persistence と propagation の追加先が tasks に明記されている。
- archive durable state の保存先・最低保持項目・resume 時の参照タイミングが design に明記されている。
- `Archiving` resume case と `Archived` terminal handoff case の両方を cover する Rust test coverage が tasks に含まれている。
- `cflx openspec validate persist-archive-resume-reasons --strict --evidence warn` が成功する。

## Out of Scope

- archive retry 回数や backoff policy 自体の全面再設計
- apply / acceptance / resolve まで含めた全 operation durable state protocol の統一
- dashboard の最終的な visual polish や blocker taxonomy 全体の統合 redesign
