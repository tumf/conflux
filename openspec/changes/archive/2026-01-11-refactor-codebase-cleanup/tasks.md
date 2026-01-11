## 1. Investigation and Planning
- [x] 1.1 Inventory command execution logic duplications and organize consolidation candidates
- [x] 1.2 Create a list of `#[allow(dead_code)]` targets and classify into delete/isolate/keep
- [x] 1.3 Confirm legacy (`opencode.rs`) usage and decide on deletion or isolation approach

## 2. Implementation (Incremental Refactoring)
- [x] 2.1 Introduce common helper for `jj` command execution, consolidate `jj_workspace.rs` and `parallel_executor.rs`
- [x] 2.2 Align `agent.rs` command execution helper with unified design, reduce duplication
- [x] 2.3 Delete or isolate legacy modules based on reference status
- [x] 2.4 Remove or narrow scope of `#[allow(dead_code)]`, document reason when kept

## 3. Verification
- [x] 3.1 Run `cargo fmt` to confirm formatting
- [x] 3.2 Run `cargo clippy -- -D warnings` to confirm no warnings
- [x] 3.3 Run `cargo test` to confirm behavior is maintained
