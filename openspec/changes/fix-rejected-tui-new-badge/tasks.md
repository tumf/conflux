## Implementation Tasks

- [x] 1. `src/tui/state.rs` の rejected row 追加・更新経路を active new-change 検出から分離し、rejected row が常に `is_new = false` を維持して `new_change_count` に含まれないようにする (verification: unit - add or update an AppState refresh test near `src/tui/state.rs` proving a newly surfaced rejected row keeps `is_new == false` and does not increment `new_change_count`)
- [x] 2. `src/tui/render.rs` の Select / Running 描画パスで rejected row に `NEW` バッジが出ないことを固定する (verification: unit - add or update render tests near `src/tui/render.rs` proving rejected rows do not include `NEW` in rendered output and still present `rejected` status)
- [x] 3. marker removal 後の rejected -> `not queued` 再活性化挙動を回帰させない (verification: unit - keep or extend TUI state tests proving rejected -> marker removal -> `not queued` reactivation still works)
- [ ] 4. proposal delta と関連実装修正の検証手順を strict validate / Rust 検証コマンドで確認する (verification: integration - run `cflx openspec validate fix-rejected-tui-new-badge --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`)

## Implementation Blocker #1
- category: other
- summary: `cargo test` 全体が本変更と無関係な既存失敗で完走しないため、Task 4 のフル検証を完了できない
- evidence:
  - `agent-exec run -- cargo test` -> `test result: FAILED. 1516 passed; 36 failed; 6 ignored`
  - 失敗例: `task_parser::* PoisonError`, `server::api::*`, `orchestrator::*` (job: `bc6b4f0e94530c886d4d530444f37e64`)
- impact: Task 4 の「cargo test 成功」条件のみ未達（strict validate / clippy / 対象回帰テストは実施済み）
- unblock_actions:
  - 既存失敗テスト群を別 change で修正し、`cargo test` をグリーン化する
  - グリーン化後に Task 4 の残条件（cargo test 成功）を再実行して完了扱いにする
- owner: maintainer
- decision_due: 2026-05-05

## Future Work

- rejected 以外の read-only terminal row に対する visual semantics の再整理
- TUI の new-badge 付与ポリシー全体を capability として一般化する再設計
