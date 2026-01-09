# Task Completion Checklist

Before marking a task as complete:

1. **Code Quality**
   - [ ] `cargo fmt` - Code is formatted
   - [ ] `cargo clippy -- -D warnings` - No linting warnings
   - [ ] `cargo build` - Code compiles

2. **Testing**
   - [ ] `cargo test` - All tests pass
   - [ ] New functionality has tests
   - [ ] Coverage not decreased (check with `cargo llvm-cov`)

3. **Documentation**
   - [ ] Code comments are in English
   - [ ] Public APIs have doc comments
   - [ ] README updated if needed

4. **OpenSpec**
   - [ ] `tasks.md` checklist updated
   - [ ] Specification scenarios covered by tests
