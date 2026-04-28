## Implementation Tasks

- [x] 1. `src/tui/render.rs` の `render_changes_list_select()` に rejected row 用の status label 描画を追加し、Select mode でも `[rejected]` を見せる (verification: unit - add or update Select mode render tests proving a rejected row includes `[rejected]` in rendered output)
- [x] 2. rejected row の `NEW` 非表示と execution mark なしの既存 semantics を Select mode 描画変更後も維持する (verification: unit - keep or extend render tests proving rejected rows still hide `NEW` and do not render as execution-marked rows)
- [x] 3. Running mode の rejected row 描画を回帰させない (verification: unit - keep or extend `src/tui/render.rs` Running mode render tests proving rejected rows still include `[rejected]` in rendered output)
- [x] 4. proposal delta と関連描画変更の検証手順を strict validate / Rust 検証コマンドで確認する (verification: integration - run `cflx openspec validate add-tui-rejected-status-label --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- Select mode における他 terminal status の label policy 整理
- change list の status / progress / badge 領域の幅配分見直し

## Acceptance #1 Failure Follow-up
- [x] Re-ran `agent-exec run -- prek run --all-files` (job `bcc4ff6da8d6c617b5a850b7a6ca8e19`) and confirmed hooks complete with `exit_code: 0`, clearing the previously reported pre-commit failure path.
- [x] Re-ran `git status --short --untracked-files=all` after the successful hook run and confirmed no remaining working-tree changes, so the dirty-tree blocker has been resolved.
