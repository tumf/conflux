## Implementation Tasks

- [ ] Task 1: Extend hook configuration parsing to accept a `git_commit_no_verify` boolean on detailed hook objects while preserving current string-form compatibility (verification: `src/hooks.rs` supports deserialization defaults and existing hook config tests still cover string/object forms)
- [ ] Task 2: Propagate `git_commit_no_verify` through hook execution into the child process environment in a stable, documented variable name (verification: hook execution code in `src/hooks.rs` sets the environment variable and unit tests verify it is present for configured hooks and absent/false by default)
- [ ] Task 3: Update hook-related specs to document the new detailed hook option, default behavior, and execution propagation semantics (verification: `openspec/changes/add-hook-git-commit-no-verify-option/specs/**/spec.md` validates in strict mode)
- [ ] Task 4: Add regression tests covering detailed hook config with `git_commit_no_verify: true` and default `false` behavior (verification: targeted Rust tests pass for config deserialization and hook environment propagation)
- [ ] Task 5: Validate the proposal strictly after authoring and ensure all proposal files are internally consistent (verification: `python3 "/Users/tumf/.agents/skills/openclaw-imports/cflx-proposal/scripts/cflx.py" validate add-hook-git-commit-no-verify-option --strict` passes)

## Future Work

- Wire repository release wrappers such as `scripts/bump.sh` to consume the propagated flag and choose `--no-verify` when desired
- Revisit whether `on_merged` should remain best-effort or gain stronger failure handling independently of this config addition
