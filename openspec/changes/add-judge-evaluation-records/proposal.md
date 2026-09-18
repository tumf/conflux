---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/judge-commands/spec.md
  - openspec/specs/configuration/spec.md
  - src/analyzer.rs
  - src/config/types.rs
  - src/config/defaults.rs
  - src/cli.rs
  - src/main.rs
  - docs/guides/CONFIG.md
verifications:
  - id: judge-evaluation-tests
    requirement: Shadow judge evaluation records are isolated, bounded, privacy-preserving, non-authoritative, and queryable through the dedicated CLI
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/analyzer.rs
    evidence: cargo test judge_evaluation --lib
    rerun: cargo test judge_evaluation --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add isolated judge evaluation records

**Change Type**: implementation

## Problem / Context

The `parallel_dependency` judge currently emits only aggregate comparison fields into Conflux's ordinary operational log. That proves a single shadow observation occurred, but it does not preserve the pair-level probabilities and authoritative analyzer labels needed to measure precision, recall, threshold behavior, failure rate, latency, or cost before promoting the judge beyond shadow mode.

Putting those records into ordinary logs would mix evaluation evidence with operator diagnostics, expose them through existing log readers and APIs, and couple their retention to the seven-day operational-log policy. The Constitution permits external metrics and logs only as non-authoritative observability outputs; deleting them must not affect routing or completion.

## Proposed Solution

Add an opt-in, product-neutral evaluation recorder under `judge_commands.parallel_dependency.evaluation`. A valid shadow observation or bounded judge failure writes one compact JSONL record beneath a dedicated evaluation root:

```text
<state-root>/evaluations/parallel_dependency/<project-slug>/<YYYY-MM-DD>.jsonl
```

The recorder shares only the existing `state_base_dir` resolution. It does not write beneath `logs`, does not enter the TUI/Web log ring, and is not read by orchestration.

Successful records contain schema/version identity, timestamp, Conflux version, purpose, mode, validated model, outcome, duration, token usage, configured threshold, and one record per directed pair. Pair records contain an installation-keyed opaque identifier, judge probability, thresholded judge label, and authoritative analyzer label. They contain no raw change IDs, proposal text, request/response body, command, environment, repository path, credential, or provider error body.

Use a locally generated 32-byte random salt stored under the evaluation root with directory mode `0700` and file mode `0600`. Pair IDs are `sha256:<hex>` values derived from the salt plus the project slug and length-prefixed directed pair identities. Salt creation is atomic; inability to create/read a valid salt disables that observation write and emits one bounded warning without affecting analysis.

Evaluation is disabled by default. Its complete configuration is part of the atomically replaced `parallel_dependency` judge entry. When enabled, `retention_days` and `max_total_bytes` are required positive bounded values. Startup cleanup applies age retention and a global evaluation-root byte budget independently, never follows symlinks, protects the current file, removes only recognized regular JSONL files, and reports partial failures without affecting orchestration.

Add a read-only `cflx judge-eval summary --purpose parallel_dependency` command that reads only the dedicated root and reports record counts, outcome counts/rates, confusion counts, precision, recall, specificity, p50/p95 latency, token totals, and grouping by model and Conflux version. It must not initialize ordinary logging, trigger cleanup, or expose raw pair identifiers by default.

## Acceptance Criteria

- Evaluation persistence is absent unless `evaluation.enabled` is explicitly true.
- Evaluation JSONL is stored outside `<state-root>/logs` and is invisible to `cflx logs`, the TUI log ring, and Web log endpoints.
- A successful shadow comparison persists the fields required to recompute confusion metrics and alternate thresholds without storing raw change IDs or request/proposal content.
- Judge failure outcomes persist bounded metadata without raw process output or provider error bodies.
- Recorder, salt, cleanup, or summary failures never alter `AnalysisResult`, provenance, scheduler routing, blockers, Acceptance, Archive, lifecycle state, or process success.
- Evaluation storage is bounded independently by age and global bytes; ordinary log retention never deletes evaluation records and evaluation cleanup never deletes ordinary logs.
- `cflx judge-eval summary --purpose parallel_dependency` deterministically summarizes fixture records and reports malformed lines as skipped/error counts rather than treating them as valid evidence.
- Focused tests cover disabled mode, success/failure records, privacy exclusions, file modes, concurrent append integrity, salt stability/corruption, symlink containment, active-file protection, age and byte cleanup, partial I/O failure, summary metrics, malformed records, and strict separation from ordinary logs.

## Explicit Completion Conditions

- Source contains a dedicated evaluation record schema, writer, retention implementation, and summary reader wired to shadow judge outcomes.
- Configuration parsing, precedence, validation, templates, and `docs/guides/CONFIG.md` describe the opt-in evaluation object and independent storage/retention contract.
- CLI parsing and dispatch expose only the read-only summary operation in this change; export and mutation commands are not implied.
- `cargo test judge_evaluation --lib` executes at least one test and passes.
- `cflx openspec validate add-judge-evaluation-records --strict --evidence warn` and `--archive-gate` pass before archive.
- After archive, the canonical `judge-commands` and `configuration` requirements contain the complete promoted contracts without losing existing scenarios.

## Out of Scope

- Using evaluation records for runtime routing, Acceptance, Archive, retry, blocker, or promotion decisions.
- Automatically enabling recording when a judge command is configured.
- `prefer`, `only`, or guarded fast-path judge modes.
- Raw request/response retention, proposal snapshots, named change-pair export, or provider-specific fields.
- Web/TUI dashboards, log API integration, background cleanup daemons, remote telemetry, or automatic model promotion.
- A general-purpose evaluation framework for purposes other than `parallel_dependency`.
