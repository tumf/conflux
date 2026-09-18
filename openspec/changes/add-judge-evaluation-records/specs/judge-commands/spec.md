## ADDED Requirements

### Requirement: Isolated durable dependency-judge evaluation evidence

When `judge_commands.parallel_dependency.evaluation.enabled` is true, Conflux SHALL write versioned evaluation JSONL outside the ordinary log root at `<state-root>/evaluations/parallel_dependency/<project-slug>/<YYYY-MM-DD>.jsonl`. Evaluation data SHALL be non-authoritative observability: absence, deletion, corruption, write failure, cleanup failure, or read failure SHALL NOT change analysis, provenance, routing, blockers, Acceptance, Archive, lifecycle state, or process success.

An observed record SHALL preserve the configured threshold, validated model, duration, usage, and each directed pair's judge probability, thresholded judge label, and authoritative analyzer label. Pair identity SHALL use a locally salted opaque SHA-256 identifier over unambiguous project-scoped directed identities. Records SHALL NOT contain raw change IDs, proposal or request content, raw command output, command strings, environment values, repository paths, credentials, or provider error bodies. Failure records SHALL contain only bounded typed outcome metadata.

Evaluation directories SHALL use mode `0700` and files SHALL use mode `0600` where supported. A 32-byte installation salt SHALL be created atomically beneath the evaluation root without replacing an already installed salt, SHALL NOT be a symlink, and SHALL never be exposed by the summary command or ordinary logs. Append operations SHALL preserve complete one-line JSON records under concurrent writers.

Failure record outcomes SHALL use the existing stable `JudgeFailureCategory::as_str()` tokens without renaming or aggregation: `spawn`, `input_too_large`, `timeout`, `cancelled`, `nonzero_exit`, `invalid_request`, `authentication`, `transient`, `output_too_large`, `invalid_json`, `schema_mismatch`, and `io`.

<!-- Expected canonical result after archive: opt-in shadow evidence is durably recorded in an isolated, privacy-preserving, non-authoritative JSONL store. -->

#### Scenario: Successful shadow comparison is independently reproducible

**Given**: evaluation recording is enabled and a valid judge observation completes before the authoritative analyzer
**When**: normalized authoritative dependencies become available
**Then**: one schema-versioned evaluation record contains every directed pair's opaque ID, probability, thresholded judge label, and analyzer label
**And**: the record contains model/version, duration, token usage, and threshold needed for aggregate evaluation
**And**: the authoritative result remains unchanged

#### Scenario: Evaluation data is separate from operational logs

**Given**: evaluation records and ordinary logs both exist under one resolved state root
**When**: an operator uses `cflx logs`, the TUI log ring, or Web log endpoints
**Then**: no evaluation record is returned
**And**: ordinary log cleanup cannot delete evaluation records
**And**: evaluation cleanup cannot delete ordinary logs

#### Scenario: Runtime recording failure is non-authoritative

**Given**: the evaluation root, salt, lock, append, or flush operation fails
**When**: dependency analysis completes
**Then**: Conflux emits at most one bounded path-free warning for the attempted operation
**And**: the conventional analyzer result and process outcome are unchanged
**And**: no raw evaluation record or sensitive input is copied into ordinary logs

#### Scenario: Evaluation artifacts omit sensitive and named inputs

**Given**: change IDs, proposals, commands, environment values, and process outputs contain distinctive secret-like fixture values
**When**: successful and failed evaluation records are inspected
**Then**: none of those fixture values occurs in record bytes, file names, directory names derived from change identity, or ordinary diagnostics
**And**: pair identity remains stable for the same installation salt, project, and direction while differing across salts, projects, or reversed directions

### Requirement: Independently bounded evaluation retention

Enabled evaluation configuration SHALL require positive `retention_days` and `max_total_bytes`, bounded respectively at 3650 days and 10 GiB. Cleanup SHALL inventory only recognized regular files under the dedicated evaluation root, SHALL NOT follow symlinks, SHALL protect the current writer file, SHALL apply age retention before a global cross-project byte budget, and SHALL evict deterministically by ascending filename date with canonical relative path as tie-breaker.

Cleanup SHALL report considered files, removed files, freed bytes, protected files, and individual failures. Partial failure SHALL preserve successful removals and SHALL NOT affect orchestration. The protected current file MAY temporarily exceed the global budget.

<!-- Expected canonical result after archive: evaluation storage is bounded independently of ordinary logs by age and global bytes without endangering active writers or unrelated files. -->

#### Scenario: Age and global bytes are independent bounds

**Given**: recognized evaluation files span multiple projects and dates
**When**: cleanup runs
**Then**: files older than the configured age are removed first
**And**: remaining non-active files are removed oldest-first until the global evaluation bytes fit the configured budget
**And**: the selected current writer is retained even when it alone exceeds the budget

#### Scenario: Unknown and unsafe entries are not deleted

**Given**: the evaluation root contains symlinks, directories, unknown files, malformed names, the installation salt, and ordinary logs outside the root
**When**: cleanup runs
**Then**: cleanup removes none of those entries
**And**: each unsafe or unrecognized entry is excluded from eligible byte accounting

### Requirement: Read-only dependency-judge evaluation summary

Conflux SHALL expose `cflx judge-eval summary --purpose parallel_dependency` with optional positive `--since-days`, exact `--project`, and `--json` selection. The command SHALL read only the dedicated evaluation root and SHALL NOT initialize ordinary logging, create state or salt files, run cleanup, or mutate repository/runtime state.

The summary SHALL report files and lines examined; valid observation and failure counts; malformed or unsupported records skipped; outcome counts and rates; true-positive, false-positive, false-negative, and true-negative pair totals; nullable precision, recall, and specificity; deterministic nearest-rank p50 and p95 duration; token totals; and counts grouped by validated model and Conflux version. JSON mode SHALL emit exactly one versioned object. Raw pair identifiers SHALL NOT appear in default human or JSON summary output.

<!-- Expected canonical result after archive: operators can inspect promotion metrics through a dedicated read-only command without mixing evaluation data into normal logs. -->

#### Scenario: Summary computes deterministic promotion metrics

**Given**: fixture records contain known labels, outcomes, durations, token usage, models, and Conflux versions
**When**: the summary command reads them
**Then**: confusion metrics, nullable rates, nearest-rank percentiles, totals, and groups equal the fixture-derived values
**And**: repeated reads produce byte-equivalent JSON output apart from no time-dependent field

#### Scenario: Malformed and future records are excluded truthfully

**Given**: recognized files contain valid records, malformed lines, and unsupported schema versions
**When**: summary runs
**Then**: malformed and unsupported lines are counted as skipped
**And**: they do not contribute to valid observations, failures, confusion metrics, latency, tokens, or model/version groups

#### Scenario: Missing data is a read-only empty result

**Given**: the selected evaluation root or purpose directory does not exist
**When**: summary runs
**Then**: it returns an empty versioned summary without creating the root
**And**: no operational log file is initialized
