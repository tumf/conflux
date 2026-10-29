## Implementation Tasks

- [ ] Introduce the shared byte-preserving LF/CRLF delimiter scanner and wire execution, strict/display and strict dependency parsing to it; prove equivalent metadata and keys, mixed delimiters, empty/EOF blocks and unclosed CRLF refusal without rewriting input bytes (verification: unit - cargo test --locked --offline --lib --features web-monitoring,heavy-tests verification_frontmatter_line_endings -- --nocapture; verification-id: frontmatter-line-endings-native)
- [ ] Exercise the production post-Apply candidate reader/manifest/gate boundary with CRLF passing, failing, wrong-typed and unclosed proposals; assert exact gate/reviewer counts and unchanged evidence on defects, preserving raw content identity and legitimate-empty behavior (verification: integration - cargo test --locked --offline --lib --features web-monitoring,heavy-tests verification_frontmatter_line_endings -- --nocapture; verification-id: frontmatter-line-endings-native)
- [ ] Add real-binary CLI regression controls for strict/archive/verify-plan/verify-run across LF, CRLF and mixed supported endings, with nonzero selections and observed command outcomes, plus focused spec/docs coherence for the shared line-ending contract (verification: integration - cargo test --locked --offline --test run_exit_tests --features web-monitoring,heavy-tests verification_frontmatter_line_endings -- --nocapture; verification-id: frontmatter-line-endings-cli)

## Final Validation

Run both declared gates and require nonzero executed tests. Run the adjacent declaration-integrity, leading-env and proposal-metadata regressions once, recording failures without broadening scope. The final structural gate is `cflx openspec validate fix-verification-frontmatter-line-endings --archive-gate`; structural validation alone is not completion evidence.

## Out of Scope

No deployment, push, release, running-owner replacement, incident-candidate edit, global configuration edit, unrelated parser logging repair or full-suite assertion weakening.
