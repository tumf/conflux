## Implementation Tasks

- [x] 1. `src/tui/render.rs` の `render_changes_list_select()` に rejected row 用の status label 描画を追加し、Select mode でも `[rejected]` を見せる (verification: unit - add or update Select mode render tests proving a rejected row includes `[rejected]` in rendered output)
- [x] 2. rejected row の `NEW` 非表示と execution mark なしの既存 semantics を Select mode 描画変更後も維持する (verification: unit - keep or extend render tests proving rejected rows still hide `NEW` and do not render as execution-marked rows)
- [x] 3. Running mode の rejected row 描画を回帰させない (verification: unit - keep or extend `src/tui/render.rs` Running mode render tests proving rejected rows still include `[rejected]` in rendered output)
- [x] 4. proposal delta と関連描画変更の検証手順を strict validate / Rust 検証コマンドで確認する (verification: integration - run `cflx openspec validate add-tui-rejected-status-label --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- Select mode における他 terminal status の label policy 整理
- change list の status / progress / badge 領域の幅配分見直し

## Acceptance #1 Failure Follow-up
- [ ] /Users/tumf/work/conflux/.git/hooks/pre-commit:1-14 and .pre-commit-config.yaml:4-26 show normal commits run prek hooks, but `agent-exec run -- prek run --all-files` failed with exit 1 after auto-fixing trailing whitespace in graphify-out/GRAPH_REPORT.md (job 6670ab48ee67a5f92d69168a383b9a96), so the real archive commit path is currently blocked until hooks pass without modifying files.
- [ ] `git status --short --untracked-files=all` remains dirty after the hook run with modified files graphify-out/GRAPH_REPORT.md, graphify-out/graph.html, and graphify-out/graph.json; this dirty working tree would block acceptance and the final archive commit path until those changes are reconciled and the tree is clean.
