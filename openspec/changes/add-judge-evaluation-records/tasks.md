## Implementation Tasks

- [ ] Extend the atomically replaced `parallel_dependency` configuration with `mode: "shadow"` and the opt-in evaluation object; validate required positive bounds and hard maximums, preserve omitted/disabled behavior, update templates and `docs/guides/CONFIG.md`, and add configuration/precedence tests. (verification: unit - `cargo test judge_evaluation --lib`, covering parse, validation, defaults, and atomic replacement in `src/config/types.rs` and related config modules; verification-id: judge-evaluation-tests)

- [ ] Add the dedicated evaluation-root resolver, schema-v1 JSONL types, installation salt handling, directional pair pseudonymization, permission enforcement, and concurrency-safe append writer without exposing raw pair identities or process/request content. (verification: unit - `cargo test judge_evaluation --lib`, covering deterministic injected salt/clock fixtures, privacy assertions, permission checks, corrupt/symlink salt handling, and concurrent complete-line append in the new evaluation module; verification-id: judge-evaluation-tests)

- [ ] Wire observed and bounded failure outcomes from the shadow dependency judge to the recorder while preserving the conventional analyzer as sole authority and swallowing all evaluation I/O failures after one bounded warning. (verification: integration - `cargo test judge_evaluation --lib`, proving observed/failure records are emitted from analyzer fixtures and record failure or root deletion leaves `AnalysisResult` and provenance unchanged in `src/analyzer.rs`; verification-id: judge-evaluation-tests)

- [ ] Implement independent age and global-byte retention for recognized evaluation JSONL files, with deterministic oldest-first eviction, current-writer protection, symlink refusal, partial-failure reporting, and no access to ordinary log files. (verification: unit - `cargo test judge_evaluation --lib`, covering multi-project age/byte eviction, active file protection, malformed/unknown entries, symlinks, and partial I/O failure in the evaluation module; verification-id: judge-evaluation-tests)

- [ ] Add read-only `cflx judge-eval summary --purpose parallel_dependency [--since-days N] [--project SLUG] [--json]`, deterministic metrics/percentiles, malformed/future-schema accounting, exact JSON output contract, and dispatch that initializes neither logging nor cleanup. (verification: integration - `cargo test judge_evaluation --lib`, exercising CLI parsing and fixture-backed human/JSON summaries through `src/cli.rs` and `src/main.rs`; verification-id: judge-evaluation-tests)

- [ ] Add separation and regression coverage proving disabled mode creates no evaluation state, `cflx logs`/TUI/Web log surfaces do not expose evaluation records, ordinary log cleanup cannot delete evaluations, evaluation cleanup cannot delete logs, and deleting the evaluation root cannot change workflow decisions. (verification: integration - `cargo test judge_evaluation --lib`, with temporary state roots and real path resolvers across logging, analyzer, and summary boundaries; verification-id: judge-evaluation-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected commands:

```text
cflx openspec validate add-judge-evaluation-records --strict --evidence warn
cflx openspec validate add-judge-evaluation-records --archive-gate
```

The repository's tracked commit hook owns unconditional repository-wide formatting and fast regression checks. The change-specific blocking gate remains `cargo test judge_evaluation --lib` because it proves the new runtime behavior directly.

## Future Work

- Guarded fast-path promotion remains a separate proposal after enough evaluation evidence exists.
- Export, purge CLI, dashboard visualization, and evaluation purposes beyond `parallel_dependency` remain separate reviewable changes.
- Real-world promotion thresholds and model selection remain operator decisions based on collected evidence; this change does not auto-promote any judge.
