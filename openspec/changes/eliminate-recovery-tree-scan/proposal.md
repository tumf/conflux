---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/upstream-integration/spec.md
  - src/main.rs
  - src/upstream/startup.rs
  - src/upstream/coordinator.rs
  - src/upstream/git_ops.rs
  - src/upstream/ports.rs
  - src/upstream/spine.rs
verifications:
  - id: upstream-recovery-tests
    requirement: Recovery discovery preserves refusal semantics while avoiding commit-tree reads
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test results for upstream recovery and Git operation regression coverage
    rerun: cargo test upstream --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: startup-performance-benchmark
    requirement: Bounded recovery discovery uses constant Git subprocess count rather than per-commit tree scans
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: benchmark output comparing recovery discovery across short and 500-commit first-parent histories
    rerun: cargo test upstream --lib --features heavy -- --ignored
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Eliminate unused recovery tree scans

**Change Type**: implementation

## Problem / Context

Default local TUI and finite `cflx run` startup must inspect Git history for unfinished upstream publication before orchestration mutates the workspace. Since v0.6.210, both recovery scans call the same first-parent API used by full upstream spine validation. That API attaches archive and active-change tree evidence to every commit by spawning two `git ls-tree` commands per commit.

The recovery scanners only consume commit SHA, parent SHAs, and message trailers. On a 500-commit history, the two scanners therefore perform about 2,000 unused Git subprocesses before terminal initialization or orchestration. Measurements on the Conflux repository show approximately 27–40 seconds before TUI output, while the Unix listener binds in milliseconds.

The recovery refusal is required for crash safety and MUST remain synchronous before orchestration. Full spine validation also MUST retain commit-tree archive evidence. The defect is the shared observation contract, not the recovery requirement itself.

## Proposed Solution

Split first-parent history observation into two explicit capabilities:

1. A metadata-only bounded walk returning SHA, parents, and raw message in oldest-first order for offline recovery discovery.
2. The existing evidence-bearing spine walk for selected upstream integration, retaining archive and active-change tree evidence for every commit that `validate_spine` classifies.

Route `scan_pending_publications` and `scan_unpushed_upstream_merges` through the metadata-only walk. Preserve their 500-commit bound, trailer parsing, merge-parent validation, local remote-tracking ref checks, and refusal diagnostics. Keep enabled upstream spine validation on the evidence-bearing path.

This remains one proposal because the API separation, recovery rewiring, and spine preservation must ship atomically: changing only one side either leaves the startup regression or weakens repository-evidence validation.

## Acceptance Criteria

- Option-less cumulative parallel TUI and `cflx run` still refuse startup before orchestration when bounded Git evidence identifies an unpublished publication marker or unpushed Conflux upstream merge.
- Recovery discovery obtains SHA, parents, and raw message without reading `openspec/changes` trees for every scanned commit.
- Recovery discovery retains the current first-parent ordering and 500-commit bound.
- Publication trailer parsing, upstream trailer parent binding, remote-tracking reachability checks, and user diagnostics remain behaviorally unchanged.
- Enabled upstream integration still validates cumulative change integrations against commit-tree archive and active-change evidence.
- Git subprocess count for ordinary no-recovery startup remains constant with respect to the number of scanned commits, excluding reachability commands for actual matching trailer evidence.
- The fix applies to both the local TUI path and finite `cflx run` path through their shared recovery check.

## Explicit Completion Conditions

- `UpstreamGit` exposes separate metadata-only recovery history and evidence-bearing spine observations with documentation matching their consumers.
- Native Git operations implement metadata recovery with one bounded `git log --first-parent` invocation and no per-commit `git ls-tree` calls.
- Both recovery scanners use the metadata-only observation; full spine validation continues to receive non-default commit-tree evidence.
- Unit or integration coverage fails if recovery scanning requests tree evidence, changes ordering or limits, misses valid refusal evidence, or accepts contradicted upstream parent trailers.
- Evidence-bearing spine tests continue to reject cumulative integration commits lacking archive evidence or retaining active change directories.
- A repository-local heavy benchmark demonstrates that no-match recovery scanning at the 500-commit bound does not grow Git subprocess count per commit and records elapsed-time context without imposing a brittle wall-clock gate.
- `cargo fmt --check`, repository lint/type checks, default tests, and the declared upstream verification commands pass.

## Out of Scope

- Removing, delaying, or making best-effort the pre-orchestration upstream recovery refusal.
- Reducing the 500-commit recovery bound.
- Replacing Git subprocesses with a new Git library.
- Optimizing the separate 2–4 second first-draw worktree/change enumeration observed between v0.6.204 and v0.6.209.
- Changing upstream publication, verification, push, or remote-confirmation semantics.
