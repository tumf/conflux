# Tasks: Fix Dynamic Queue Ingestion Repo-Root Resolution and Self-Referential Test Fixture

## Implementation Tasks

- [x] Task 1: Switch dynamic-queue ingestion candidate validation to the configured
      repository root: in
      `src/parallel/queue_state.rs::check_dynamic_queue_and_add_changes`
      (currently line 1645), replace `crate::openspec::list_changes_native()` with
      `crate::openspec::list_changes_native_from(&self.repo_root)`. Do not touch the
      other `list_changes_native()` call sites. Completion condition:
      `rg -n "list_changes_native\(\)" src/parallel/queue_state.rs` matches only the
      retry-body site at ~line 1224 (on_merged hook task counts), not the ingestion
      site. verification: unit - covered by Task 3's repo-root resolution test, which
      fails if the ingestion site regresses to cwd-based lookup

- [x] Task 2: Make `scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve`
      (`src/parallel/tests/manual_resolve.rs:284`) self-contained: create a `TempDir`
      containing a synthetic ACTIVE change at
      `openspec/changes/<synthetic-id>/` (minimal `proposal.md` + `tasks.md` so
      `list_changes_native_from` lists it), construct the executor with
      `repo_root = tempdir path` instead of `PathBuf::from(".")`, and push the
      synthetic id to the dynamic queue. Keep all original assertions: dynamic-ingest
      log for the synthetic id, `AnalysisStarted` within the 500ms window,
      `dispatch_capacity_zero_after_analysis` diagnostic, gate counter still held, no
      `ApplyStarted`. Completion condition: the test contains no real change id of this
      repository (in particular not `fix-spawned-retry-lane-release`) and passes on the
      current main tree where that change is archived; runs under 1 second.
      verification: integration - `cargo test --lib parallel::tests::manual_resolve::scheduler_loop_ingests_dynamic_queue_during_gated_manual_resolve`
      (red on current main before this change, green after)

- [x] Task 3: Add a focused ingestion repo-root regression test in
      `src/parallel/tests/` driving `check_dynamic_queue_and_add_changes` (directly or
      via the scheduler loop) with an executor whose `repo_root` is a temp dir that is
      NOT the process cwd: (a) a change present only under the temp `repo_root` is
      ingested into `queued` and emits the "Dynamically added to parallel execution"
      log; (b) a candidate id absent under that `repo_root` is not queued and emits the
      `candidate_not_found` reconciliation log. Completion condition: test (a) fails
      when Task 1 is reverted to cwd-based `list_changes_native()`; both cases run
      under 1 second. verification: unit - new test(s) in
      `src/parallel/tests/manual_resolve.rs` or `src/parallel/tests/executor.rs`

- [x] Task 4: Run quality gates on the final tree. Completion condition: all of the
      following exit 0, and no existing manual-resolve, auto-resolve, executor, drain,
      or capacity-gating test regresses.
      verification: integration - `cargo test --lib parallel::tests` plus
      `cargo test --lib orchestration::state`, `cargo fmt --check`, and
      `cargo clippy --locked --all-targets --all-features -- -D warnings`

## Future Work

- Migrate the remaining cwd-based `list_changes_native()` call sites (orchestrator,
  main, TUI, web, merge paths) to explicit `repo_root`-based resolution (verification:
  manual - tracked as a future change proposal; each site needs its own cwd-vs-root
  risk assessment).

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-dynamic-queue-ingest-repo-root --archive-gate`
