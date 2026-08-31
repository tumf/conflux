## 1. Resolution contract

- [ ] 1.1 Add a fail-closed archived-proposal resolver in `src/archive_layout.rs` beside `find_valid_archive_entry`, reusing `is_valid_archive_entry_name` and leaving `find_valid_archive_entry` and its callers untouched. (verification: unit - `cargo test --lib archive_layout` covers direct archive, dated archive, duplicate valid entries, nested layout, suffix collision, and an entry without `proposal.md`; verification-id: archived-change-verification-resolution)
- [ ] 1.2 Route `cflx openspec verify` declaration loading through the resolver with unconditional active precedence, keeping the executor workspace at `std::env::current_dir()`. (verification: unit - `cargo test --lib openspec_cmd::verify`; verification-id: archived-change-verification-resolution)

## 2. Regression coverage

- [ ] 2.1 Prove one archived declaration retains its verification ID and can plan or execute repository-level automation after archive. (verification: unit - archive-only regression in `src/openspec_cmd/verify.rs` passes via `cargo test --lib openspec_cmd::verify`; verification-id: archived-change-verification-resolution)
- [ ] 2.2 Prove an active proposal still resolves when a same-named archive entry exists, and that invalid or duplicate archive identities fail closed with actionable diagnostics. (verification: unit - `cargo test --lib archive_layout` and `cargo test --lib openspec_cmd::verify` cover precedence, invalid nested, suffix collision, and duplicate entries; verification-id: archived-change-verification-resolution)

## 3. Contract validation

- [ ] 3.1 Update canonical proposal-metadata behavior for archive-aware verification resolution. (verification: manual - source path `openspec/changes/verify-archived-change/specs/proposal-metadata/spec.md` shows active precedence and archived resolution without frontmatter field changes; verification-id: archived-change-verification-resolution)
- [ ] 3.2 Run focused tests and strict validation. (verification: integration - `cargo test --lib` and `cflx openspec validate verify-archived-change --strict --evidence error` provide repository-verifiable output; verification-id: archived-change-verification-resolution)
