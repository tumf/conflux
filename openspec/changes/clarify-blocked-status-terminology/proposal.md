---
change_type: implementation
priority: high
dependencies:
  - separate-apply-block-from-reject
references:
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/frontend-abstraction/spec.md
  - openspec/changes/separate-apply-block-from-reject/proposal.md
  - openspec/changes/classify-acceptance-followup-routing/proposal.md
  - src/parallel/queue_state.rs
  - src/orchestration/state.rs
  - src/execution/apply.rs
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/server/api/ws.rs
  - src/web/state.rs
  - src/tui/state.rs
---

# Change: blocked 用語を dependency wait 専用にし stalled / acceptance-blocked を分離する

**Change Type**: implementation

## Premise / Context

- 現行仕様と実装では、dependency analysis による `blocked`、apply/rejecting 側の resumable blocker、acceptance verdict の blocker が会話上も UI 上も近い語で表現されている。
- `src/parallel/queue_state.rs` と `src/orchestration/state.rs` では dependency wait が reducer 由来の `blocked` 表示になる一方、`openspec/specs/parallel-execution/spec.md` では permission auto-reject が `stalled/blocked` と表現されている。
- active proposal `separate-apply-block-from-reject` は現在、apply/rejecting 側の resumable hold を `blocked` として実装・仕様化しており、dependency wait 用の blocked と衝突しうる。
- active proposal `classify-acceptance-followup-routing` は acceptance follow-up の reroute を扱うが、acceptance gate を dependency blocked と別語で固定する canonical taxonomy は未定義である。

## Requested Artifact

- implementation proposal for status terminology separation across reducer, runtime, UI, and specs
- canonical taxonomy where `blocked` means dependency wait only, while apply-side resumable hold and acceptance blocker use distinct names

## Problem / Context

Conflux の現在の状態語彙では、少なくとも三つの異なる現象が `blocked` 近傍の語で表現されている。1つ目は dependency analysis によって dispatch できない queued change、2つ目は apply 中に権限 auto-reject や追加情報待ちで一時停止すべき resumable hold、3つ目は acceptance verdict として実装 blocker が観測されたケースである。

この衝突により、仕様レビュー・実装・UI 表示・ログ解釈のすべてで「何が blocked なのか」が曖昧になる。dependency wait は scheduler 上の待機理由なのに、apply/resume hold や acceptance blocker と同じ語で表されると、queue semantics、failed-change tracking、resume policy、frontend mapping の境界が崩れる。canonical spec と reducer/display contract を明示的に分離し、`blocked` を dependency wait 専用語へ固定する必要がある。

## Proposed Solution

status taxonomy を以下のように再定義する。

- `blocked` は dependency wait 専用語とし、未解決依存により queued change を dispatch できない状態だけに使う。仕様本文ではこの概念を `dependency-blocked` と明示する。
- apply / rejecting review 由来の resumable hold は `stalled` とし、change は non-terminal のまま worktree・WIP・reason metadata を保持して再開可能とする。
- acceptance parser / acceptance-follow-up 由来の gate failure は `gated` として観測し、仕様本文ではこの概念を `acceptance-gated` と明示する。
- `separate-apply-block-from-reject` が現在 apply-side hold に `blocked` を使っている前提を踏まえ、この proposal は dependency-blocked / acceptance-gated を先に固定し、そのうえで apply-side hold を `stalled` へ寄せる移行条件を明記する。
- reducer、runtime event、TUI/Web/API、log wording、active proposal assumptions をこの taxonomy にそろえる。
- failed-change tracking と dependency skip 判定は `stalled` を failure-side signal として扱い続ける一方、dependency wait の `blocked` は queue wait reason として維持する。

## Acceptance Criteria

- dependency unresolved により queued change が dispatch 待ちになる場合のみ、derived display status と user-facing wording は `blocked` を使う。
- apply permission auto-reject や同等の resumable blocker は `stalled` として記録・表示され、dependency wait の `blocked` と同一 status 名にならない。
- acceptance verdict と follow-up classification が implementation blocker を観測した場合、少なくとも logs / events / UI / test contracts のいずれかで `gated` として区別される。
- TUI / Web / API / reducer tests は `blocked`、`stalled`、`gated` の三者を区別し、frontend が独自に collapse しないことを確認できる。
- `separate-apply-block-from-reject` と `classify-acceptance-followup-routing` が前提にしている blocked terminology は canonical taxonomy と矛盾しない形に更新される。

## Explicit Completion Conditions

- OpenSpec delta が dependency blocked、apply stalled、acceptance-blocked の canonical meaning と責務境界を明記している。
- tasks が reducer/runtime/event/frontend/logging/test coverage の更新箇所を具体的な repository evidence 付きで列挙している。
- active change との整合方針（少なくとも `separate-apply-block-from-reject` と `classify-acceptance-followup-routing`）が proposal または design に記録されている。
- `cflx openspec validate clarify-blocked-status-terminology --strict --evidence warn` が成功する。

## Out of Scope

- blocked/stalled badge の最終的な visual polish や色設計全体の見直し
- archive retry taxonomy まで含めた全 failure vocabulary の統合 redesign
- acceptance protocol 自体の全面再設計
