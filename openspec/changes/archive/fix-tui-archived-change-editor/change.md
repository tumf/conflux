---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/utils.rs
  - src/tui/key_handlers.rs
  - openspec/specs/cli/spec.md
---

# Fix TUI archived change editor launch

**Change Type**: implementation

## Problem / Context

- TUI の `e` キーは `src/tui/key_handlers.rs` から `launch_editor_for_change()` を呼び出して change の proposal もしくは change directory を開く。
- 既存実装は `openspec/changes/<change_id>` の active change しか探索せず、archive 済み change が存在する `openspec/changes/archive/` を見ない。
- そのため archive 済み change を TUI 上で選択して `e` を押しても editor launch が `ChangeNotFound` になり、archive 済み proposal/spec を確認できない。
- 既に `src/tui/utils.rs` では archive directory fallback を追加して修正済みであり、この挙動を OpenSpec proposal として明文化しておく必要がある。

## Proposed Solution

- TUI の change editor launch は active change に加えて `openspec/changes/archive/` 配下の archive 済み change も探索対象にする。
- archive entry は direct match (`archive/<change_id>`) と date-prefix 付き entry (`archive/<date>-<change_id>`) の両方を許容する。
- `proposal.md` が存在すればそのファイルを開き、存在しなければ解決した change directory を editor の cwd として開く、という既存 fallback を archive change に対しても維持する。
- 実装には archived change fallback を保護する unit test を含める。

## Acceptance Criteria

- TUI Changes view で archive 済み change を選択して `e` を押すと、該当 archive entry の `proposal.md` もしくは change directory が開かれる。
- archive entry 名が `<change_id>` でも `<date>-<change_id>` でも同じ change として解決される。
- active change の既存 editor launch 挙動は維持される。
- `src/tui/utils.rs` の unit test が active / archived の両パスを検証する。

## Out of Scope

- TUI の右ペイン表示や act/exp 表示ロジック全体の再設計
- Worktrees view における editor launch 挙動の変更
- archive 済み change の spec 内容表示 UI の新設
