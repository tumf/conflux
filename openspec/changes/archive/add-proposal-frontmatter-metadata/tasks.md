## Implementation Tasks

- [x] Task 1: OpenSpec proposal metadata 規約を追加する (verification: `openspec/changes/add-proposal-frontmatter-metadata/specs/proposal-metadata/spec.md` に frontmatter と各フィールド定義がある)
- [x] Task 2: `src/openspec.rs` の proposal parser を拡張し、frontmatter の `dependencies` を優先しつつ本文 `## Dependencies` への後方互換を維持する (verification: `src/openspec.rs` のテストで frontmatter 優先・本文 fallback の両方が通る)
- [x] Task 3: proposal metadata を表す共有データ構造を追加し、`priority` と `references` を保持できるようにする (verification: metadata 抽出の単体テストが追加される)
- [x] Task 4: frontmatter parser に unknown key warning を追加し、既知キー以外では失敗せず警告を返すようにする (verification: unknown key を含む proposal のテストで parsing は成功し warning が返る)
- [x] Task 5: `src/server/proposal_session.rs` の proposal 読み取りを拡張し、title に加えて metadata を安全に抽出できるようにする (verification: proposal session 用テストで frontmatter 付き proposal から metadata を取得できる)
- [x] Task 6: proposal 作成指示と fixture を frontmatter + `references` 前提に更新する (verification: `skills/cflx-proposal/SKILL.md`、`.claude/commands/cflx/proposal.md`、`skills/tests/fixtures/proposal_modes/*/proposal.md` が整合する)
- [x] Task 7: proposal/frontmatter 関連テストと検証コマンドを更新する (verification: `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` が通る)

## Future Work

- `priority` を analyzer / scheduler の実行優先度に反映するか検討する
- Dashboard / Web API に `references` をそのまま露出するか検討する
