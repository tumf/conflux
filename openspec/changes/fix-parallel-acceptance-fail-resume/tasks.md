## Implementation Tasks

- [x] 1. Make `skip_apply` mutable and consume it after the first acceptance-only cycle so subsequent cycles always enter apply (verification: `rg "skip_apply" src/parallel/dispatch.rs` shows `let mut skip_apply` and a reset to `false`)
- [x] 2. Add unit/integration test: resumed workspace with acceptance FAIL triggers a second cycle that enters apply (verification: `cargo test --test e2e_tests` or new test in `src/parallel/tests/`)
- [x] 3. Run `cargo fmt --check && cargo clippy -- -D warnings && cargo test` to confirm no regressions
