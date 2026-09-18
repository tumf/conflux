### Requirement: Optional purpose-scoped judge command

Conflux SHALL support the optional product-neutral `judge_commands.parallel_dependency` configuration defined by the `configuration` capability for one bounded typed-judgment subprocess.

The configured command SHALL receive one TypeSafe System One request object on stdin and SHALL return one TypeSafe System One response object on stdout. Conflux SHALL invoke the argv directly without shell interpolation and SHALL NOT manage or inject provider-specific credentials. Runtime judge semantics SHALL remain governed by this capability; field validation and precedence SHALL remain governed by `configuration`.

Compatibility tests and examples SHALL use `jev-cli v0.4.1`, tag commit `50488f35d705051b5d621d1d10e69d1503783e7d`, as the immutable baseline for `jev run -`, stdout/stderr separation, and exit codes. Runtime configuration SHALL remain product-neutral and MAY use any command conforming to that contract.

<!-- Expected canonical result after archive: configuration exposes one optional, product-neutral, atomically overridden System One-compatible dependency judge command. -->

#### Scenario: Existing configuration has no judge command

**Given**: the merged configuration omits `judge_commands`
**When**: Conflux loads and runs dependency analysis
**Then**: configuration validation succeeds
**And**: no judge process is started
**And**: the existing analyzer behavior is unchanged

#### Scenario: Purpose-scoped command uses argv and a pinned model

**Given**: `judge_commands.parallel_dependency.command` is `["jev", "run", "-"]`
**And**: its model is `jev-1.13.0`
**When**: configuration is loaded
**Then**: Conflux retains the command as an argv array without shell parsing
**And**: every request uses `jev-1.13.0`

#### Scenario: Baseline does not float with repository main

**Given**: compatibility fixtures are generated or reviewed
**When**: the reference command behavior is selected
**Then**: the source is the immutable `jev-cli v0.4.1` tag commit `50488f35d705051b5d621d1d10e69d1503783e7d`
**And**: later changes on `jev-cli` repository `main` do not silently change Conflux's accepted command contract

#### Scenario: Higher-priority entry replaces the complete trust boundary

**Given**: a lower-priority layer configures one parallel-dependency command, model, and limits
**And**: a higher-priority layer supplies another `parallel_dependency` entry
**When**: configurations are merged
**Then**: the higher-priority entry replaces the complete lower-priority entry
**And**: no executable, model, threshold, timeout, or byte limit is inherited field-by-field from the replaced entry

#### Scenario: Invalid command bounds fail configuration loading

**Given**: an enabled parallel-dependency judge has an empty argv, empty model, zero timeout, zero byte limit, excessive bound, non-finite threshold, or threshold outside `[0.5,1.0]`
**When**: configuration validation runs
**Then**: loading fails with an actionable error naming the invalid field

### Requirement: Strict System One machine protocol

For each eligible multi-change dependency analysis, Conflux SHALL build one deterministic TypeSafe System One request containing structured queued and in-flight change state and one directed `noul` question for every permitted queued-dependent and queued-or-in-flight dependency-candidate pair.

Question IDs SHALL be deterministic index-based identifiers mapped to change pairs in trusted local memory. Change IDs SHALL NOT be used as JSON question keys. Proposal metadata dependencies SHALL remain separately authoritative.

Conflux SHALL accept a response only when stdout contains exactly one JSON value, the returned model matches the configured request model, the answer IDs exactly match the request, every answer has `type: "noul"`, every `noul` value is finite and in `[0,1]`, and usage counters are non-negative integers. Noul values SHALL NOT be interpreted as carrying a separate confidence field.

<!-- Expected canonical result after archive: dependency judges use a deterministic, strictly validated native System One request/response protocol with bounded data and trusted local pair mapping. -->

#### Scenario: Batched directed dependency questions

**Given**: two queued changes and one explicitly supplied in-flight change
**When**: Conflux builds the shadow request
**Then**: one request contains every permitted directed pair whose dependent is queued and candidate dependency is another queued or in-flight change
**And**: each question asks only whether the dependent requires repository output produced by the candidate dependency
**And**: priority, references, and mere topical relation are explicitly excluded as dependency criteria

#### Scenario: Untrusted change ID cannot change protocol structure

**Given**: a change ID contains characters meaningful in JSON paths or object keys
**When**: the request is built
**Then**: the question key is derived only from trusted integer indexes
**And**: the original change ID appears only as a state value

#### Scenario: Malformed response is a shadow failure

**Given**: command stdout has prose, multiple JSON values, trailing non-whitespace, a wrong model, missing or extra answer IDs, wrong answer types, invalid usage, or non-finite or out-of-range Noul values
**When**: Conflux parses the response
**Then**: it rejects the response as a bounded shadow failure
**And**: it does not alter or fail the conventional dependency analysis

#### Scenario: Oversized input or output is bounded

**Given**: serialized input exceeds the configured input limit or command output exceeds the configured output limit
**When**: shadow execution is attempted
**Then**: Conflux skips or terminates the judge as applicable
**And**: the conventional dependency analysis continues unchanged

### Requirement: Non-authoritative concurrent dependency shadow

The parallel-dependency judge SHALL run concurrently with the existing multi-change `analyze_command`. The conventional analyzer result, after existing normalization, metadata dependency union, and validation, SHALL remain the sole authority for order and dependency routing.

Conventional analyzer completion SHALL be the judge's return deadline. When the judge has not already completed, Conflux SHALL signal cancellation, transfer complete process ownership to a detached cleanup task, and return the conventional result without awaiting cleanup. The cleanup task SHALL send TERM immediately, wait exactly 100 milliseconds, send KILL if needed, and reap the child; process drop SHALL also be configured to kill as a runtime-shutdown backstop. Judge success, failure, uncertainty, cancellation, or output SHALL NOT change `AnalysisResult`, `AnalysisProvenance`, scheduler routing, blocker classification, Acceptance, Archive, or lifecycle state.

Judge failures SHALL produce only redacted, bounded observability. Diagnostics SHALL NOT contain request state, proposal text, credentials, environment values, raw stdout, raw stderr, provider error bodies, or individual named judgments.

<!-- Expected canonical result after archive: the System One dependency judge is a concurrent shadow observer that can neither delay nor influence the authoritative analyzer. -->

#### Scenario: Slow judge cannot delay conventional analysis

**Given**: the configured judge remains running after the conventional analyzer has completed
**When**: the conventional result is ready
**Then**: Conflux transfers cleanup ownership to a detached task
**And**: cleanup uses the fixed 100 millisecond TERM-to-KILL grace and reaps the process group
**And**: Conflux returns the conventional result without awaiting cleanup or the judge timeout

#### Scenario: Judge completes first

**Given**: the judge returns a valid response before the conventional analyzer completes
**When**: the conventional normalized result later becomes available
**Then**: Conflux compares thresholded judge edges with the conventional dependency edges for aggregate observability
**And**: it returns the conventional result unchanged

#### Scenario: Judge command is unavailable or unhealthy

**Given**: the executable is missing, credentials are absent, spawn fails, the process exits non-zero, times out, exceeds limits, crashes, or returns invalid output
**When**: dependency analysis runs
**Then**: the existing analyzer path completes with its prior semantics
**And**: no change becomes blocked, stalled, or failed because of the judge

#### Scenario: Judge is skipped outside the eligible analyzer path

**Given**: judge configuration is absent, `use_llm_analysis` is false, there are zero changes, there is one queued change, or no permitted pair exists
**When**: dependency analysis runs
**Then**: Conflux does not spawn the judge
**And**: all existing early-return and metadata-only behavior is preserved

#### Scenario: Proposal metadata remains authoritative

**Given**: proposal metadata declares a dependency
**And**: the judge Noul value for that pair is below the configured yes threshold
**When**: conventional analysis is normalized
**Then**: the metadata dependency remains in the authoritative `AnalysisResult`
**And**: the judge disagreement is observability only

#### Scenario: Diagnostics are aggregate and redacted

**Given**: a valid judge observation or bounded judge failure occurs
**When**: Conflux emits diagnostics
**Then**: diagnostics may include purpose, outcome category, duration, answer counts, validated model identity, and aggregate edge comparison counts
**And**: diagnostics omit all request content, raw process output, secrets, and individual named pair values

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
