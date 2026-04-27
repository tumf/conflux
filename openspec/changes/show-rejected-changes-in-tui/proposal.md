---
change_type: implementation
priority: medium
dependencies:
  - clear-rejected-selection-mark
references:
  - src/openspec.rs
  - src/tui/state.rs
  - src/tui/runner.rs
  - src/orchestration/state.rs
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/tui-state/spec.md
---

# Change: TUI change 一覧に rejected change を read-only 表示する

**Change Type**: implementation

## Premise / Context

- ユーザー要望は「REJECTED な change が TUI 一覧に表示されないので、rejected 状態で表示してほしい」「ただし x マークは付けられない」である。
- 現行実装では `src/openspec.rs` の `list_changes_native()` が `REJECTED.md` を持つ change を active execution list から除外している。
- canonical spec でも rejected marker-bearing change は execution-oriented active listing から除外する契約を持つ一方、read-only operational surface では表示してよい流れがすでに dashboard 側で定義されている。
- TUI は change 一覧更新に active execution list 相当の入力へ強く依存しているため、rejected change を read-only row として復元できない。
- `clear-rejected-selection-mark` proposal は rejected change の execution mark clear semantics を扱うため、本 proposal はそれを前提に TUI 可視化と操作ガードを追加する。

## Requested Artifact

- implementation

## Problem / Context

現状の TUI では `REJECTED.md` を持つ change が一覧に出ないため、rejection flow により terminal `rejected` になった change を CLI 上で確認できない。

この状態では、base branch に durable rejection marker が残っていても、ユーザーは TUI から rejected outcome を追跡できない。また execution candidate list と read-only operational list の責務が分離されていないため、今後 rejected row を表示しても queue 操作や x マーク semantics が混ざりやすい。

## Proposed Solution

TUI change 一覧は execution-oriented active listing と同一視せず、read-only rejected row を併合できる表示用 snapshot を持つ。

- execution candidate discovery と `cflx run` の入力には、引き続き `REJECTED.md` を除外する active listing を使う
- TUI の表示更新では、`proposal.md` と `REJECTED.md` を持つ change を read-only rejected row として追加できるようにする
- rejected row の display status は `rejected` とし、色は reducer vocabulary に合わせる
- rejected row は `selected = false` を維持し、Space / `@` / `F5` / resume 系操作で x マークや queue intent を付けられない
- `REJECTED.md` が base branch から消えた場合のみ、次回 refresh で通常の active change として `not queued` かつ unselected へ戻る

## Acceptance Criteria

1. `openspec/changes/<change-id>/proposal.md` と `openspec/changes/<change-id>/REJECTED.md` が存在するとき、TUI change 一覧にその change が read-only row として表示される。
2. 上記 row の display status は `rejected` であり、reducer/TUI status vocabulary と整合した色・表示になる。
3. rejected row に対して Space や他の queue 操作を行っても x マークは付かず、`selected` や queue intent は変化しない。
4. rejected row は一覧に表示されても execution candidate にはならず、`cflx run` や TUI の再開/実行開始フローへ混入しない。
5. `REJECTED.md` を削除して refresh すると、その change は通常の active change として `not queued` かつ `selected = false` から再活性化される。
6. strict validation が通る spec delta と、TUI state / refresh reconciliation / queue guard の回帰テスト計画が追加される。

## Out of Scope

- rejected reason 本文の新しい detail panel 表示
- dashboard / server API の rejected row 契約変更
- archived / merged / error など他 terminal row の selection semantics 再設計
