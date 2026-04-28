## Implementation Tasks

- [x] 1. `src/tui/state.rs` の rejected row 追加・更新経路を active new-change 検出から分離し、rejected row が常に `is_new = false` を維持して `new_change_count` に含まれないようにする (verification: unit - add or update an AppState refresh test near `src/tui/state.rs` proving a newly surfaced rejected row keeps `is_new == false` and does not increment `new_change_count`)
- [x] 2. `src/tui/render.rs` の Select / Running 描画パスで rejected row に `NEW` バッジが出ないことを固定する (verification: unit - add or update render tests near `src/tui/render.rs` proving rejected rows do not include `NEW` in rendered output and still present `rejected` status)
- [x] 3. marker removal 後の rejected -> `not queued` 再活性化挙動を回帰させない (verification: unit - keep or extend TUI state tests proving rejected -> marker removal -> `not queued` reactivation still works)
- [x] 4. proposal delta と関連実装修正の検証手順を strict validate / Rust 検証コマンドで確認する (verification: integration - run `cflx openspec validate fix-rejected-tui-new-badge --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- rejected 以外の read-only terminal row に対する visual semantics の再整理
- TUI の new-badge 付与ポリシー全体を capability として一般化する再設計

## Rejecting Recovery Tasks

- [x] Investigate blocker in openspec/changes/fix-rejected-tui-new-badge/REJECTED.md and implement a non-rejection recovery path before rerunning apply (verification: integration - rerun `cflx openspec validate fix-rejected-tui-new-badge --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`)
