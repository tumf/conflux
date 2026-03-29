## Implementation Tasks

- [ ] Task 1: analyze における proposal frontmatter 利用ルールを spec delta に追加する (verification: `openspec/changes/update-analyze-use-proposal-frontmatter/specs/parallel-analysis/spec.md` に dependencies / priority / references の役割が定義される)
- [ ] Task 2: `src/analyzer.rs` で analyze 入力へ proposal metadata を渡す経路を追加する (verification: analyzer prompt 構築テストで frontmatter metadata がプロンプトに反映される)
- [ ] Task 3: frontmatter `dependencies` を analyze dependency source として優先し、本文 `## Dependencies` を fallback にする (verification: frontmatter あり/なし両方の analyzer テストが通る)
- [ ] Task 4: frontmatter `priority` を order のソフトヒントとして扱い、dependency graph には影響させないことをテストで固定する (verification: 同一依存条件の changes で priority が順序ヒントとして反映されるテストがある)
- [ ] Task 5: frontmatter `references` を補助コンテキストとして analyze prompt に含める (verification: analyzer prompt テストで references セクションが出力される)
- [ ] Task 6: unknown frontmatter key warning があっても analyze が継続するケースを追加する (verification: warning 付き metadata を持つ proposal の analyzer テストが通る)
- [ ] Task 7: analyzer 関連テストと検証コマンドを更新する (verification: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` が通る)

## Future Work

- `priority` を analyzer の出力 order だけでなく scheduler の実行枠選択に使うか検討する
- Web UI / API から analyze 用 metadata を可視化するか検討する
