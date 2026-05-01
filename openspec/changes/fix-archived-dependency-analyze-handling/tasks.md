## Implementation Tasks

- [ ] 1. Classify dependency targets in `src/analyzer.rs` as queued, in-flight, archived, or missing before validating graph edges. (verification: unit - `cargo test analyzer::tests::test_archived_dependency_reference_is_normalized_without_parse_failure` exercises archived-only dependency input)
- [ ] 2. Normalize archived dependency references out of the executable dependency graph while preserving queued and in-flight dependency edges. (verification: unit - `cargo test analyzer::tests::test_archived_dependency_reference_is_normalized_without_parse_failure` asserts `src/analyzer.rs` returns `AnalysisResult.dependencies` with only queued/in-flight edges)
- [ ] 3. Keep missing dependency references as dedicated invalid dependency failures with diagnostics that distinguish missing from archived. (verification: unit - `cargo test analyzer::tests::test_missing_dependency_reference_still_fails_with_missing_classification` asserts `src/analyzer.rs` returns an error containing missing classification context)
- [ ] 4. Ensure user-facing analyze parse errors do not label archived dependency references as generic invalid JSON. (verification: integration - `cargo test analyzer` covers `src/analyzer.rs` archived dependency diagnostics and asserts the archived case does not contain `Analysis returned invalid JSON`)

## Future Work

- Consider a separate proposal to auto-prune archived dependency metadata after archive if repeated authoring confusion continues.
