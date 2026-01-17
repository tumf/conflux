## 1. Implementation
- [x] 1.1 Implement git merge-tree based conflict detection in src/vcs/git/commands.rs
- [x] 1.2 Replace check_merge_conflicts call in src/tui/runner.rs with new merge-tree based function
- [x] 1.3 Update existing tests for check_merge_conflicts function
- [x] 1.4 Verify worktree creation/deletion flow is not affected

## 2. Validation
- [x] 2.1 npx @fission-ai/openspec@latest validate avoid-merge-abort-conflict-check --strict
- [x] 2.2 cargo test (related unit tests only)
