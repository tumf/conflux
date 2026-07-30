## Implementation Tasks

- [ ] **Task 1: Introduce typed explicit-target classifications** for active, already-completed, resumable-worktree, unknown, duplicate, and evidence-error results while retaining original requested order. (verification: unit - target resolver table tests named `explicit_target_resume_*` in `src/orchestrator.rs` run by `cargo test explicit_target_resume` cover every class and aggregate diagnostics; verification-id: explicit-target-resume-tests)

- [ ] **Task 2: Resolve base-integrated completion from captured base-branch tree evidence** by reusing/factoring `is_merged_to_base()` for exact/date-prefixed archive entries, active-directory absence, attached branch identity, and command errors without commit-message or runtime-state inference. (verification: integration - temporary Git repository cases in `tests/e2e_git_worktree_tests.rs` run via the declared heavy command and fail for archive-only or subject-only stubs; verification-id: explicit-target-resume-tests)

- [ ] **Task 3: Validate managed-worktree resume evidence** using existing worktree discovery plus worktree-local change/archive/Git state before registering a requested target; reject name-only, missing-path, contradictory, or unreadable candidates without cleanup or replacement creation. (verification: e2e - `tests/e2e_git_worktree_tests.rs` real worktree fixtures run by `cargo test --features heavy-tests --test e2e_git_worktree_tests explicit_target_resume` cover applied, archiving, archived-not-integrated, malformed, stale-name-only, and missing worktree states; verification-id: explicit-target-resume-tests)

- [ ] **Task 4: Replace active-list-only filtering for explicit parallel targets** so `Orchestrator` captures base identity, resolves all targets before mutation, sends active/resumable targets into existing scheduling, and records already-completed IDs as successful skips; retain duplicate and unknown aggregation. (verification: e2e - repeated-invocation cases in `tests/e2e_git_worktree_tests.rs`, run by the declared heavy command, process only remaining/resumable work and do not return unknown for base-integrated IDs; verification-id: explicit-target-resume-tests)

- [ ] **Task 5: Define `--no-resume`, dry-run, `--all`, serial, and `-u` boundaries**: base-completed remains skipped under `--no-resume`, worktree-only recovery fails without deleting it, dry-run performs read-only classification, `--all`/serial remain compatible, and upstream mode consumes the shared resolver. (verification: e2e - option-boundary cases in `tests/e2e_git_worktree_tests.rs`, run by the declared heavy command, exercise each combination and assert no side effects for dry-run/error paths; verification-id: explicit-target-resume-tests)

- [ ] **Task 6: Expose ordered classification evidence to output consumers** so human dry-run output and typed terminal consumers can distinguish requested, processed/resumable, already-completed, and pending/unknown IDs without parsing diagnostics. (verification: integration - resolver output tests in `src/orchestrator.rs` run by `cargo test explicit_target_resume` assert stable ordered arrays and no server/runtime journal input; verification-id: explicit-target-resume-tests)

## Future Work

- Serial mode may adopt the same repository-evidence target resolver in a separate change if serial execution remains supported.
- A supervisor may persist the original requested set and resubmit it unchanged; no cflx workflow state is added to the supervisor contract.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate resume-explicit-completed-targets --archive-gate`

Repository quality gates expected before acceptance: `cargo test --features heavy-tests --test e2e_git_worktree_tests explicit_target_resume`, `cargo fmt --check`, and `cargo clippy --all-targets --all-features -- -D warnings`. The real Git/worktree suite remains behind `heavy-tests` and outside the default test suite.
