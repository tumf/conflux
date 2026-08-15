## Implementation Tasks

- [ ] Add a regression harness for `ResolveFailed` followed by manual `M` retry with a conflict-free, identity-verified target `MERGE_HEAD`; assert the retry reaches sequential resolve rather than returning `MergeDeferred`. (verification-id: manual-resolve-recovery) (verification: integration - `cargo test --lib manual_resolve -- --nocapture`)
- [ ] Route admitted manual resolve intent past the generic base-dirty preflight into repository-derived sequential resolve classification while preserving exclusive base-lane ownership. (verification-id: manual-resolve-recovery) (verification: unit - `cargo test --lib manual_resolve -- --nocapture`)
- [ ] Cover valid completion and fail-closed foreign, ambiguous, conflicted, and unrelated-dirty states; assert invalid evidence leaves Git state unchanged and valid completion clears `MERGE_HEAD`. (verification-id: manual-resolve-recovery) (verification: integration - `cargo test --lib manual_resolve -- --nocapture`)

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate fix-manual-resolve-unfinished-merge --archive-gate`
