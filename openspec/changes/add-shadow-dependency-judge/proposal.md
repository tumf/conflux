---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/analyzer.rs
  - src/config/types.rs
  - src/parallel_run_service.rs
  - openspec/specs/configuration/spec.md
  - openspec/specs/parallel-analysis/spec.md
verifications:
  - id: shadow-dependency-judge-tests
    requirement: Optional purpose-scoped judge commands are configuration-compatible and cannot alter or delay authoritative parallel analysis
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/lib.rs
    evidence: cargo test --lib
    rerun: cargo test --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add optional shadow dependency judge

**Change Type**: implementation

## Problem / Context

`ParallelizationAnalyzer` currently relies on `analyze_command` to produce the authoritative execution order and inferred dependency graph, then unions proposal metadata dependencies and validates the result. Conflux has no narrow command boundary for a fast typed judgment service.

A local `jev` CLI can accept a complete TypeSafe System One request on stdin and return the native System One response on stdout. Conflux should be able to invoke that CLI for dependency judgments without naming Jev or TypeSafe in configuration or source contracts. Equivalent future products are expected to implement the same request and response schema.

The judge must remain optional and non-authoritative. Existing configuration, existing `analyze_command` behavior, metadata dependency authority, validation, scheduler routing, and recoverable analyzer fallback must remain unchanged when the judge is absent or fails.

## Proposed Solution

Add an optional top-level `judge_commands.parallel_dependency` configuration entry. Its command is an argv array, not a shell template. Conflux sends one batched TypeSafe System One request to the command's stdin for all ordered pairs in the current queued and in-flight working set and accepts exactly one System One response object on stdout.

The first integration is shadow-only:

1. For a multi-change analysis eligible for the existing `analyze_command`, start the conventional analyzer future and the configured dependency-judge future concurrently.
2. The conventional analyzer remains authoritative and is returned unchanged.
3. If the judge completes first with a valid response, compare its thresholded dependency edges with the conventional analyzer's normalized result and emit bounded diagnostics only.
4. If the conventional analyzer completes before the judge, terminate and reap the judge process immediately and return the conventional result without waiting for the judge timeout.
5. Missing configuration, spawn errors, unavailable executables, authentication errors, non-zero exits, timeouts, oversized output, malformed JSON, schema/type/range mismatches, missing answers, or uncertain values never fail, block, reorder, or delay the conventional analysis path.

The command owns provider communication and credentials. Conflux passes no provider API key and does not require `jev` to be installed.

## Acceptance Criteria

- Existing configuration without `judge_commands` loads and behaves exactly as before.
- `judge_commands.parallel_dependency` accepts a non-empty argv command, a pinned non-alias request model, a bounded positive timeout, bounded positive input/output byte limits, and a probability threshold in `[0.5, 1.0]`.
- Higher-priority configuration replaces the complete `parallel_dependency` entry; it does not merge individual command fields across trust boundaries.
- The judge receives one TypeSafe System One request containing structured queued/in-flight change state and one `noul` question for each ordered candidate dependency pair.
- Question IDs are deterministic indexes whose pair mapping is retained locally; untrusted change IDs are not embedded in question IDs.
- The response parser requires the configured model field, the exact expected answer ID set, `type: "noul"`, finite `noul` values in `[0, 1]`, and bounded stdout. Extra answer IDs, missing answer IDs, multiple top-level JSON values, prose, and trailing non-whitespace are rejected as shadow failures.
- Proposal metadata dependencies remain authoritative. Judge output never removes, adds, or changes an `AnalysisResult` dependency or order.
- The conventional `analyze_command` and judge command run concurrently. When the judge would outlive the conventional analyzer, Conflux signals cancellation and transfers child ownership to a detached cleanup task before returning the conventional result. That task sends TERM immediately, waits the fixed 100 ms judge cleanup grace, then sends KILL if needed and reaps the child. Enabling a slow or unavailable judge therefore does not hold the authoritative result behind cleanup.
- Judge diagnostics contain purpose, outcome category, duration, pair/edge counts, and model identity where valid. They do not contain request state, proposal text, credentials, raw stdout, raw stderr, or provider error bodies.
- Judge failures do not produce `blocked`, `stalled`, or `failed` change lifecycle state and do not alter `AnalysisProvenance`.
- `use_llm_analysis = false`, zero changes, one queued change, or no candidate pair skips the judge and preserves existing behavior.

## Explicit Completion Conditions

- Configuration structs, precedence/merge logic, validation, and examples cover the optional purpose-scoped command.
- A dedicated judge runner performs argv-based spawn, exact stdin request write, bounded stdout/stderr collection, timeout, cancellation, and process reaping without using `AiCommandRunner` or shell interpolation. Its owned detached cleanup task plus `kill_on_drop` cover cancellation and runtime shutdown.
- `ParallelizationAnalyzer` creates the deterministic batched request, races shadow execution against conventional analysis, strictly parses valid responses, compares shadow and authoritative edges, and never consumes shadow output as routing input.
- Repository-local tests use causal fake executables or injected runners to prove success, missing executable, non-zero exit, timeout, cancellation/reaping, malformed output, wrong/missing/extra answer IDs, non-finite/out-of-range values, oversize input/output, and unchanged authoritative results.
- `cargo test --lib` passes and reports non-zero executed tests covering configuration, judge runner/protocol, analyzer integration, and documentation fixtures.

## Out of Scope

- Replacing or bypassing `analyze_command`.
- A `prefer`, `only`, fail-closed, or judge-authoritative mode.
- Acceptance, error classification, finding matching, priority scoring, Apply, Archive, or blocker lifecycle integration.
- Circuit breakers, retries, durable judge state, caches, billing controls, or provider SDK dependencies.
- TypeSafe API-key storage or environment injection.
- Logging raw provider requests, responses, proposal content, stdout, or stderr.
- Changing explicit proposal metadata dependency authority or existing analyzer fallback semantics.

## Why one change

The generic command contract and its first `ParallelizationAnalyzer` consumer must ship atomically. A command configuration with no causal consumer is not independently useful, while the consumer cannot be implemented or verified without the exact request, response, timeout, and failure contract.
