## 1. Resolution contract

- [ ] 1.1 Add a shared fail-closed resolver for active and canonical archived proposal identities. (verification: unit - focused tests in `src/archive_layout.rs` cover active, direct archive, dated archive, duplicate archive, active/archive ambiguity, nested layout, and missing proposal cases; verification-id: archived-change-verification-resolution)
- [ ] 1.2 Route `cflx openspec verify` declaration loading through the shared resolver without changing repository-root execution. (verification: unit - `cargo test --lib openspec_cmd::verify`; verification-id: archived-change-verification-resolution)

## 2. Regression coverage

- [ ] 2.1 Prove one archived declaration retains its verification ID and can plan or execute repository-level automation after archive. (verification: unit - archive-only command regression in `src/openspec_cmd/verify.rs` passes via `cargo test --lib openspec_cmd::verify`; verification-id: archived-change-verification-resolution)
- [ ] 2.2 Prove invalid and ambiguous archive identities fail closed with actionable diagnostics. (verification: unit - resolver regressions in `src/archive_layout.rs` cover invalid nested, suffix-collision, duplicate, and active/archive ambiguity; verification-id: archived-change-verification-resolution)

## 3. Contract validation

- [ ] 3.1 Update canonical proposal-metadata behavior for archive-aware verification resolution. (verification: manual - source path `openspec/changes/verify-archived-change/specs/proposal-metadata/spec.md` shows active and archived resolution without frontmatter field changes; verification-id: archived-change-verification-resolution)
- [ ] 3.2 Run focused tests and strict validation. (verification: integration - `cargo test --lib openspec_cmd::verify` and `cflx openspec validate verify-archived-change --strict --evidence error` provide repository-verifiable output; verification-id: archived-change-verification-resolution)
