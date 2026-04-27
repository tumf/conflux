---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/orchestration/state.rs
  - src/tui/state.rs
  - src/web/state.rs
  - src/server/api/control.rs
  - dashboard/src/components/ChangeRow.tsx
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/server-api/spec.md
  - openspec/specs/cli/spec.md
  - openspec/specs/web-monitoring/spec.md
---

# Change: REJECTED change の execution mark を clear する

**Change Type**: implementation

## Premise / Context

- ユーザー要望は「REJECTED になった change は x マークを外す」である。
- 現行 canonical spec では rejected change は active listing から除外され、`REJECTED.md` 削除時のみ `not queued` として再活性化される (`openspec/specs/orchestration-state/spec.md`)。
- TUI / Web の `selected` は execution mark / checkbox state であり、UI 固有状態として保持される一方、実行候補の意味づけは reducer と server selection semantics に依存する (`openspec/specs/frontend-abstraction/spec.md`, `openspec/specs/server-api/spec.md`)。
- 既存コードには rejected → not queued の再活性化テストはあるが、rejected 確定時に `selected=false` へ戻す契約は明文化されていない。
- dashboard は rejected row を read-only terminal row として表示しうるため、terminal rejected row が execution mark を保持し続けると UI 意味論が崩れる。

## Requested Artifact

- implementation

## Problem / Context

change が rejection flow により `Rejected` terminal state へ遷移しても、フロントエンド側の execution mark (`selected`, checkbox, x マーク) が保持されると、実行対象から外れた terminal row がまだ「実行マーク済み」に見える。

この状態は次の不整合を生む。

- reducer / active listing 上は再キュー不可なのに、UI 上は queued candidate のように見える
- dashboard の read-only rejected row と checkbox semantics が衝突する
- 後で `REJECTED.md` を削除して再活性化したとき、本来は `not queued` から再スタートすべき change に古い execution mark が残りうる

## Proposed Solution

`Rejected` terminal state への遷移を、execution mark clear の契機として統一する。

- rejection confirm / `ChangeRejected` 適用時に、対象 change の execution mark は `selected=false` へ clear する
- この clear は対象 change のみに適用し、他 change の mark は保持する
- rejected row の display status は `rejected` のまま維持する
- dashboard / server selection state でも rejected row は read-only terminal row として扱い、`selected=true` を保持しない
- `REJECTED.md` 削除による再活性化後は、既存仕様どおり `not queued` かつ unselected から再開する

## Acceptance Criteria

1. change が rejection flow 完了により `Rejected` terminal state へ遷移したとき、TUI / Web / server snapshot で当該 change の `selected` は `false` になる。
2. 上記遷移で、他 change の `selected` 状態は変化しない。
3. rejected change の display status は `rejected` のままであり、execution mark clear によって `not queued` へ降格しない。
4. dashboard で表示される rejected row は read-only terminal row として扱われ、active execution 用 checkbox semantics を保持しない。
5. `REJECTED.md` を削除して refresh 後に change が active listing へ戻った場合、既存仕様どおり `not queued` かつ `selected=false` から再活性化される。
6. strict validation が通る spec delta と、TUI / reducer / server selection semantics を固定する回帰テスト計画が追加される。

## Out of Scope

- rejected 理由本文の新しい表示 UI
- rejected change を archive 配下へ移す保管ポリシー変更
- error / stopped / archived など他 terminal state の mark clear semantics 再設計
