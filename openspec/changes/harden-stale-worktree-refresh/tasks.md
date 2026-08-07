## Implementation Tasks

- [x] Apply one automatic observation policy before spawning Git commands in both TUI `load_worktrees_with_conflict_check` and Web/UDS `refresh_from_disk` through `observe_worktrees`, using current active/rejected change identities while retaining every Git-registered worktree with an explicit not-inspected state. Completion: stale, archived, unrelated, and branchless secondary worktrees remain visible but cannot be presented as conflict-free/mergeable merely because inspection was skipped. (verification: integration - `cargo test --locked --test e2e_git_worktree_tests`; verification-id: stale-worktree-refresh-regression)
- [x] Preserve operator control with fresh targeted observation before merge/delete eligibility is decided. Completion: periodically skipped worktrees, including `ws-session-*`, can still be inspected for operator-initiated merge or deletion; unknown observations produce an inspection-required diagnostic rather than a false no-commits-ahead message. (verification: integration - `cargo test --locked --test e2e_git_worktree_tests`; verification-id: stale-worktree-refresh-regression)
- [x] Add one process-local observation cache shared by both periodic paths, keyed by branch identity, base HEAD, worktree HEAD, and merge base, and invalidate on any key change. Completion: identical TUI/Web refresh inputs execute one merge simulation process-wide; each individual identity/revision change executes exactly one fresh inspection; cache deletion or process restart changes no workflow action for identical workspace state. (verification: integration - `cargo test --locked --test e2e_git_worktree_tests`; verification-id: stale-worktree-refresh-regression)
- [x] Bound merge simulation results and diagnostics in `src/vcs/git/commands/merge.rs`. Completion: conflict and non-conflict outcomes preserve exit status, total output byte counts, conflict count, at most 20 deterministic conflict paths, and at most 4096 bytes of each output prefix, while no tracing record or returned error contains complete arbitrarily large stdout/stderr. (verification: unit - focused merge parser/log-shaping tests plus `cargo test --locked --test e2e_git_worktree_tests`; verification-id: stale-worktree-refresh-regression)
- [x] Add stale-worktree refresh regression coverage with real temporary Git worktrees and an injected command recorder or PATH-scoped Git shim. Completion: actual command counts prove both periodic paths skip non-active worktrees, share unchanged active/rejected observations, invalidate on base/worktree/branch changes, operator actions revalidate skipped and `ws-session-*` worktrees, diagnostics obey fixed bounds, and refresh leaves index and dirty files byte-identical. (verification: integration - `cargo test --locked --test e2e_git_worktree_tests`; verification-id: stale-worktree-refresh-regression)

## Future Work

- An operator-triggered backup-and-remove action may be proposed separately; this change intentionally performs no automatic destructive cleanup.

## Notes

- Shared layer: `src/worktree_ops/inspection.rs` holds the eligibility policy, the revision-keyed observation cache, and the command recorder. Both periodic paths reach it through `worktree_ops::observe_worktrees(repo_root, ObservationRequest::Periodic)`; `src/tui/worktrees.rs` now delegates rather than duplicating the loop.
- Operator control: `WorktreeService::locate` asks for `ObservationRequest::Target(path)`, so merge and delete always re-derive ahead/conflict evidence for the addressed worktree even when periodic refresh skipped it. Worktree creation asks for `ObservationRequest::Listing`, which buys no simulation at all.
- Cache key includes the canonical repository path alongside branch identity, base HEAD, worktree HEAD, and merge base. Repository scoping can only prevent reuse, never cause a stale one; the cache is capped at 512 entries and discarded wholesale on overflow.
- Only complete observations are cached: a failed ahead or conflict command leaves the tuple unkeyed so the failure is retried rather than remembered.
- Recorder: `worktree_ops::inspection::record_inspection_commands()` is off by default (one relaxed atomic load per inspected worktree in production) and attributes each spawned command to its worktree, so the regressions assert on commands actually spawned and each filters to its own temporary repository.
- Bounded diagnostics: `check_merge_conflicts` now returns `MergeSimulation`. Its `summarize_merge_tree`/`bounded_prefix` helpers are pure and unit-tested without Git. The stdout conflicted-file parser also accepts Git's tab-separated form and reads the `CONFLICT (...)` message section from stdout, which is where `merge-tree` actually writes it — without that, every real conflict degraded to `<unknown>`.
- Presentation: `WorktreeInfo.inspection` and `WorktreeResource.inspection` (additive, `serde(default)`) let the TUI and the operator console distinguish checked, cached, and not-inspected rows. An uninspected row is never labelled `merged`, and `web/app.js` renders "Not inspected" instead of "Commits ahead of base: No".
- Fail-closed: `classify_merge_eligibility` refuses an uninspected observation with an inspection-required reason, and separates that from both "no commits ahead" and "could not be determined". The TUI merge guard deliberately does *not* block an uninspected row — blocking there would make periodic filtering permanently unmergeable — and lets the service decide from its own fresh observation.

## Final Validation

Verification evidence:

- `cargo test --locked --features heavy-tests --test e2e_git_worktree_tests` — 66 passed, 0 failed (the four new `stale_refresh_e2e_*` cases run in 1.6s). The file is `#![cfg(feature = "heavy-tests")]`, so the declared `cargo test --locked --test e2e_git_worktree_tests` compiles the same target with zero cases; the feature-enabled run is the one that carries evidence.
- `cargo test --locked` — full default suite.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` — clean.
- `cargo fmt --all -- --check` — clean.
- `npm --prefix tests/web test` — 180 passed (operator console rendering).

Run `cflx check-conflicts`, `cflx openspec validate harden-stale-worktree-refresh --strict`, `cflx openspec validate harden-stale-worktree-refresh --evidence error`, and `cflx openspec validate harden-stale-worktree-refresh --archive-gate`. Archive validation remains the authoritative final OpenSpec gate.
