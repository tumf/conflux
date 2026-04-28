---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/state.rs
  - src/tui/render.rs
  - openspec/specs/tui-state/spec.md
  - openspec/specs/cli/spec.md
  - openspec/specs/orchestration-state/spec.md
---

# Change: rejected TUI row must not appear as NEW

**Change Type**: implementation

## Premise / Context

- 現セッションでは、`REJECTED.md` を持つ change が TUI 起動時に一覧へ表示されるが、`rejected` の read-only row ではなく `NEW` バッジ付きで見える不整合が報告されている。
- canonical spec では rejected row は `rejected` の read-only row として表示され、`selected = false` を維持し、execution candidate にならないことが既に定義されている。
- 現行 TUI 実装では `src/tui/state.rs` が rejected row を refresh 時に新規追加する際 `is_new = true` を付与し、`src/tui/render.rs` が `is_new` を見て無条件に `NEW` バッジを描画している。
- その結果、status 自体は `rejected` でも、初回表示で rejected row が新規実行候補のように見えてしまう。

## Problem / Context

rejected row は operational visibility のために一覧へ残してよいが、execution candidate と誤認される visual treatment を持ってはならない。現状は rejected row に `is_new` が付くため、TUI 起動直後や refresh 直後に `NEW` バッジが描画され、既存 spec の read-only / non-candidate semantics と矛盾している。

この状態では、rejected row が「新しく見つかった通常 change」のように見え、ユーザが queue 可能な対象と誤認しうる。status 同期や別 row の実行開始後に `[rejected]` が見えても、初期視覚表現の不整合は残る。

## Proposed Solution

rejected row を TUI の「new change」検出対象から分離し、refresh で `REJECTED.md` marker-bearing row を追加・更新する場合は `is_new = false` を保証する。

- active change の新規検出と rejected display row の新規追加を別扱いにする
- rejected row 追加時は常に `selected = false` と `display_status_cache = "rejected"` を維持する
- rejected row は `new_change_count` に含めない
- TUI の Select / Running 両表示で rejected row に `NEW` バッジが出ないことを確認する
- marker removal で通常 active row に戻る既存 reactivation semantics は維持する

## Acceptance Criteria

- `proposal.md` と `REJECTED.md` を持つ change が TUI refresh で一覧に追加されても、row は `rejected` 表示となり `NEW` バッジは表示されない
- rejected row は refresh 後も `selected = false` を維持し、execution candidate として数えられない
- `new_change_count` は rejected row の追加で増えない
- Select view と Running view の両方で rejected row に `NEW` バッジが出ない
- `REJECTED.md` を削除して通常 active row に戻る既存の再活性化挙動は壊れない

## Explicit Completion Conditions

- `src/tui/state.rs` の rejected row refresh / 新規追加ロジックから、rejected row が `is_new = true` になる経路が除去されている
- rejected row の `NEW` バッジ非表示と `new_change_count` 非加算を検証する TUI state / render 系テストが追加または更新されている
- canonical spec を補う delta が strict validation を通過している
- `cargo test` の対象テストと lint/typecheck 相当の Rust 検証コマンドが成功している

## Out of Scope

- rejected row 自体を change list から再度除外する仕様変更
- rejected / archived 以外の terminal row の visual redesign
- rejection flow や reducer terminal state semantics の変更
