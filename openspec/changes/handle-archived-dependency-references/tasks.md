## Implementation Tasks

- [x] 1. Canonical spec に archived dependency reference contract を追加し、active / in-flight / archived / missing の各 dependency target の意味を明文化する。 (verification: integration - `openspec/specs/parallel-execution/spec.md` と関連 spec delta に requirement/scenario が追加され、`cflx openspec validate handle-archived-dependency-references --strict --evidence warn` が成功する)
- [x] 2. `cflx openspec validate` 経路で active proposal frontmatter の dependency targets を分類し、archived dependency references を generic missing dependency と区別して報告する。 (verification: integration - validation coverage が `src/openspec_cmd.rs` または関連 validator 経路で archived dependency case を検出し、missing case と別の outcome/message を返すことを確認する)
- [x] 3. `src/analyzer.rs` の dependency validation と outer error shaping を更新し、archive 済み dependency 由来の failure を generic `Analysis returned invalid JSON` に潰さず dedicated dependency-contract diagnostics として surfacing する。 (verification: unit - analyzer tests が `src/analyzer.rs:214-238` 相当の error shaping を通しても archived dependency root cause が保持されることを確認する)
- [x] 4. queued / in-flight / archived / missing dependency の4分類 regression tests を追加し、`separate-apply-block-from-reject` のような archived prerequisite を参照する active proposal 相当ケースで current misleading failure が再発しないことを固定する。 (verification: unit/integration - analyzer/validation tests が queued と in-flight は pass、archived は specで定義した expected outcome、missing は dedicated invalid dependency failure になることを確認する)
- [x] 5. active proposal authoring guidance を更新し、archive 後に dependency metadata をどう保守するかを repository evidence 付きで明示する。 (verification: manual - proposal/skill/spec review で active proposal author が archived dependency を残してよい条件または除去すべき条件を repo 内ドキュメントから判断できる)
- [x] 6. proposal と関連実装変更をまとめて検証する。 (verification: integration - `cflx openspec validate handle-archived-dependency-references --strict --evidence warn` と、touchした analyzer/validation tests、および必要な lint/type checks が成功する)

## Future Work

- archived dependency references を archive 時に自動リライトする仕組みの検討
- dependency analysis fallback policy を order-preserving degradation mode へ拡張する検討
