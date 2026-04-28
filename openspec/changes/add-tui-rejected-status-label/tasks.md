## Implementation Tasks

- [ ] 1. `src/tui/render.rs` の `render_changes_list_select()` に rejected row 用の status label 描画を追加し、Select mode でも `[rejected]` を見せる (verification: unit - add or update Select mode render tests proving a rejected row includes `[rejected]` in rendered output)
- [ ] 2. rejected row の `NEW` 非表示と execution mark なしの既存 semantics を Select mode 描画変更後も維持する (verification: unit - keep or extend render tests proving rejected rows still hide `NEW` and do not render as execution-marked rows)
- [ ] 3. Running mode の rejected row 描画を回帰させない (verification: unit - keep or extend `src/tui/render.rs` Running mode render tests proving rejected rows still include `[rejected]` in rendered output)
- [ ] 4. proposal delta と関連描画変更の検証手順を strict validate / Rust 検証コマンドで確認する (verification: integration - run `cflx openspec validate add-tui-rejected-status-label --strict --evidence warn`, `cargo test`, and `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- Select mode における他 terminal status の label policy 整理
- change list の status / progress / badge 領域の幅配分見直し
