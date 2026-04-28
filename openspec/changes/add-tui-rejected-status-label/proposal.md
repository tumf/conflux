---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/render.rs
  - openspec/specs/tui-state/spec.md
  - openspec/specs/cli/spec.md
---

# Change: show rejected status label in TUI select mode

**Change Type**: implementation

## Premise / Context

- 現セッションでは、rejected row に付いていた `NEW` バッジは除去されたが、TUI Select mode では row 自体に `[rejected]` ラベルが表示されないことが確認された。
- `src/tui/render.rs` の `render_changes_list_select()` は checkbox / change id / badges / task progress を描画するが、status text 自体を描画していない。
- 一方 `render_changes_list_running()` は `[rejected]` を含む status label を描画しており、モードによって rejected row の視覚表現が不一致になっている。
- canonical spec では rejected row の display status は `rejected` とされ、execution candidate ではない read-only row として扱うことが要求されている。

## Problem / Context

Select mode で rejected row に status label が出ないため、`NEW` を消した後の row は通常の change とほぼ同じ見た目になる。これでは rejected row が queue 不可の read-only terminal row であることが視覚的に伝わらず、ユーザは row の意味を判断しづらい。

Running mode では `[rejected]` が出るため、同じ `display_status_cache = "rejected"` を持つ row が mode によって異なる表現になるのも一貫性を欠く。TUI は rejected row を operational visibility のために残す以上、Select mode でも terminal status が明示される必要がある。

## Proposed Solution

Select mode の change row 描画にも status label 領域を追加し、rejected row では `[rejected]` を明示表示する。

- `render_changes_list_select()` が rejected row を通常 row と視覚的に区別できるようにする
- rejected row の status label は Running mode と同じ語彙 `rejected` を使う
- rejected row に `NEW` バッジが出ない既存挙動は維持する
- rejected row の `selected = false` / queue 不可 semantics は変更しない
- Select / Running の両 mode で rejected row が一貫して `[rejected]` と見えることを確認する

## Acceptance Criteria

- TUI Select mode で `display_status_cache = "rejected"` の row は `[rejected]` ラベルを表示する
- rejected row は Select mode でも `NEW` バッジを表示しない
- TUI Running mode での rejected row 表示は既存どおり `[rejected]` を維持する
- rejected row の execution mark なし (`selected = false`) / queue 不可 semantics は変わらない
- Select mode の他 status row の可読性が極端に崩れない

## Explicit Completion Conditions

- `src/tui/render.rs` の Select mode 描画経路に rejected status label を描くコードが追加されている
- Select mode で rejected row が `[rejected]` を表示し、`NEW` を表示しないことを確認する描画テストが追加または更新されている
- Running mode の rejected row 描画を回帰させないテストが維持または更新されている
- canonical spec を補う delta が strict validation を通過している
- 関連 Rust テストと lint/typecheck 相当コマンドが成功している

## Out of Scope

- rejected row の queue / reducer semantics の変更
- archived / merged / error など他 terminal status の Select mode 表示設計の全面見直し
- change list レイアウト全体の再設計
