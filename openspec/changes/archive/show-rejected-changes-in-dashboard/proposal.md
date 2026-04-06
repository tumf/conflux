---
change_type: hybrid
priority: medium
dependencies: []
references:
  - src/server/api/ws.rs
  - src/openspec.rs
  - src/orchestration/state.rs
  - dashboard/src/api/types.ts
  - dashboard/src/components/ChangeRow.tsx
  - dashboard/src/components/ChangesPanel.tsx
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/server-api/spec.md
---

# Change: Show rejected changes in dashboard

**Change Type**: hybrid

## Premise / Context

- ユーザー要望は「rejected な change が frontend に表示されない」を解消すること。
- `dashboard/src/api/types.ts` と `dashboard/src/components/ChangeRow.tsx` は `rejected` status の型・見た目をすでに持っている。
- 一方で `src/openspec.rs` の active change listing は `REJECTED.md` を持つ change を除外しており、`src/server/api/ws.rs` の dashboard change list も `proposal.md` 前提で active change 相当のみを列挙している。
- `src/orchestration/state.rs` は `TerminalState::Rejected(_)` から display status `rejected` を導出できるため、一覧APIが rejected change を供給すれば TUI/Web の status semantics と整合する。
- 実行対象からの除外と dashboard 上での可視化は別の契約として扱う必要がある。

## Problem / Context

現状の dashboard では `rejected` status の描画定義は存在するが、`REJECTED.md` を持つ change が一覧ソースに含まれないため、frontend では rejected change 自体が見えない。

この結果、acceptance blocker により rejection された change は durable marker として base branch に記録されていても、ユーザーは WebUI 上でその存在や理由付き terminal outcome を確認できない。これは「実行候補からは除外するが、運用上の状態としては可視であるべき rejected change」を hidden state にしてしまう。

## Proposed Solution

Dashboard 用 change listing は、実行対象の active listing とは分離して `REJECTED.md` marker を持つ change も列挙対象に含める。

- `cflx run` や queue 候補の判定に使う native active listing の exclusion semantics は維持する
- dashboard / WebSocket 用の change snapshot では、`proposal.md` が存在する change に加えて `REJECTED.md` を持つ change も返す
- `REJECTED.md` が存在する change の status は orchestrator reducer 状態の有無にかかわらず `rejected` を優先する
- rejected change は表示専用 terminal row として扱い、active execution 用 UI 制御（Stop & dequeue など）は表示しない
- restart 後や reducer state 不在時でも `REJECTED.md` により rejected change が継続して見えるようにする

## Acceptance Criteria

- `openspec/changes/<change-id>/proposal.md` と `openspec/changes/<change-id>/REJECTED.md` が base branch に存在するとき、dashboard change list はその change を含む
- 上記 change の status は WebSocket payload 上で `rejected` になる
- rejected change は `cflx run` / native active listing の候補には引き続き含まれない
- dashboard row は `rejected` の visual treatment で描画され、active change 専用操作は表示されない
- reducer state が空でも `REJECTED.md` だけで rejected row が再構築される
- strict validation が通る spec delta が追加される

## Out of Scope

- rejected change の理由本文を dashboard row に表示する追加UI
- rejected marker を archive へ移動する保管ポリシー変更
- TUI の active queue semantics 自体の変更
