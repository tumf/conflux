# Design: optional shadow dependency judge

## Goals

- Add one product-neutral, purpose-scoped command boundary for typed dependency judgments.
- Reuse the TypeSafe System One wire schema directly so Jev and compatible later tools need no Conflux-specific adapter.
- Preserve the current `ParallelizationAnalyzer` result, validation, fallback, and scheduler behavior under every judge outcome.
- Ensure a slow or failed optional judge cannot add latency to the authoritative analyzer path.

## Non-goals

The design does not create a general plugin framework. It does not make judge output authoritative, add provider credentials to Conflux, or replace `analyze_command`.

## Configuration

Add these serializable types in `src/config/types.rs`:

```rust
pub struct JudgeCommandsConfig {
    pub parallel_dependency: Option<ParallelDependencyJudgeConfig>,
}

pub struct ParallelDependencyJudgeConfig {
    pub command: Vec<String>,
    pub model: String,
    pub timeout_ms: Option<u64>,
    pub max_input_bytes: Option<usize>,
    pub max_output_bytes: Option<usize>,
    pub yes_threshold: Option<f64>,
}
```

`ParallelizationAnalyzer` gains an explicit `repo_root: PathBuf` constructor argument. `ParallelRunService` passes its existing canonical repository root, and direct test constructors pass their temporary repository root. The implementation SHALL NOT infer the root from process cwd or run `git rev-parse` during analysis.

`OrchestratorConfig` gains:

```rust
pub judge_commands: Option<JudgeCommandsConfig>
```

Defaults used after deserialization:

| Field | Default |
|---|---:|
| `timeout_ms` | `30000` |
| `max_input_bytes` | `1048576` |
| `max_output_bytes` | `1048576` |
| `yes_threshold` | `0.85` |

Validation rules:

- `command` contains at least one non-empty element; every element is free of NUL.
- `model` is non-empty after trimming and is not an alias ending in `-latest`; this first integration requires a pinned concrete model identity so strict response equality is meaningful.
- `timeout_ms` is in `1..=300000`.
- input/output limits are in `1..=16777216`.
- threshold is finite and in `0.5..=1.0`.

The existing config precedence applies to `judge_commands`, but `parallel_dependency` is an atomic trust boundary: when a higher-priority layer supplies it, that whole entry replaces the lower-priority entry. Fields are not merged. Omission inherits the lower-priority entry. This prevents a project layer from replacing only the executable while silently inheriting another layer's model or limits.

Example:

```jsonc
{
  "judge_commands": {
    "parallel_dependency": {
      "command": ["jev", "run", "-"],
      "model": "jev-1.13.0",
      "timeout_ms": 30000,
      "max_input_bytes": 1048576,
      "max_output_bytes": 1048576,
      "yes_threshold": 0.85
    }
  }
}
```

There is no `mode` field. This first contract is always shadow-only. A future authoritative mode requires a separate reviewed proposal.

## Wire contract

### Request

Conflux serializes exactly one JSON object and writes it to stdin. The command is started from the repository root with inherited environment plus existing Conflux `envs`; Conflux adds no TypeSafe-specific variables.

```json
{
  "state": {
    "schema_version": 1,
    "queued_changes": [
      {
        "index": 0,
        "id": "change-a",
        "proposal": "complete proposal.md text",
        "metadata_dependencies": [],
        "priority": "medium",
        "references": []
      }
    ],
    "in_flight_changes": [
      {
        "index": 1,
        "id": "change-b",
        "proposal": "complete proposal.md text",
        "metadata_dependencies": [],
        "priority": null,
        "references": []
      }
    ]
  },
  "model": "jev-1.13.0",
  "questions": {
    "q_0000_0001": {
      "type": "noul",
      "instructions": "Does queued change `state.queued_changes[0]` require repository output produced by in-flight change `state.in_flight_changes[0]` before it can be implemented or verified correctly?",
      "criteria": {
        "true": "The source change, contract, schema, migration, configuration, test surface, specification, or generated artifact produced by the candidate dependency is required.",
        "false": "The changes are independent, merely related, ordered only by preference or priority, or connected only by a reference."
      }
    }
  }
}
```

The implementation builds one deterministic list of nodes: queued changes in current input order, then in-flight changes in their supplied order. Each node receives an integer index. It asks one directed question for each permitted pair where:

- the dependent is queued;
- the candidate dependency is another queued or explicitly supplied in-flight change;
- dependent and candidate differ.

Question ID is `q_{dependent_index:04}_{dependency_index:04}`. The ID-to-pair map remains in memory and is the only parser authority. Change IDs never become JSON object keys.

The complete `proposal.md` text is included for queued and in-flight nodes. For every node, the path is exactly `repo_root/openspec/changes/<validated-change-id>/proposal.md`, using the explicit `repo_root` held by `ParallelizationAnalyzer`. Any absent, unreadable, non-regular, or oversized proposal file causes the complete shadow request to be skipped with the redacted `io` or `input_too_large` category. It never substitutes `null`, partially judges the set, or affects conventional analysis. Existing parsed dependency metadata is included separately and remains authoritative outside the judge.

Noul `criteria` is part of the accepted TypeSafe System One API contract. The official API reference defines optional `criteria.true` and `criteria.false` strings for Noul questions: <https://docs.typesafe.ai/api#question-types>. `jev run -` passes complete request JSON through unchanged, so its convenience `noul` subcommand omitting optional criteria does not narrow the `run` protocol.

Before spawn, serialized request size is checked against `max_input_bytes`. Oversize input yields a shadow diagnostic and skips spawn.

### Response

The command must emit one compact or pretty-printed TypeSafe System One response object to stdout:

```json
{
  "model": "jev-1.13.0",
  "answers": {
    "q_0000_0001": {
      "type": "noul",
      "noul": 0.93
    }
  },
  "usage": {
    "input_tokens": 123,
    "output_tokens": 8
  }
}
```

Strict parser rules:

- stdout must contain exactly one JSON value followed only by whitespace;
- top-level object must contain string `model`, object `answers`, and object `usage`;
- returned `model` must equal the configured request model;
- answer IDs must equal the expected ID set exactly;
- every answer must have `type: "noul"` and a finite numeric `noul` in `[0, 1]`;
- `usage.input_tokens` and `usage.output_tokens` must be non-negative integers;
- stdout beyond `max_output_bytes` is terminated and rejected;
- stderr is bounded by the same output limit, never parsed as an answer, and never logged verbatim.

TypeSafe Noul has no separate confidence field. A dependency edge is a shadow `yes` when `noul >= yes_threshold`; otherwise it is a shadow `no`. Thresholding exists only for comparison metrics and never modifies `AnalysisResult`.

## Runtime placement and concurrency

Keep `analyze_command` as the authoritative owner. Integrate at the multi-change branch in `ParallelizationAnalyzer::analyze_with_callback`:

1. Build the existing conventional analysis future.
2. When `judge_commands.parallel_dependency` is valid and there are permitted pairs, build the shadow request and start a dedicated child process.
3. Poll both concurrently.
4. Conventional analysis completion is the return deadline:
   - if judge has already completed, retain its parsed shadow observation;
   - otherwise signal cancellation, move the complete child/process-group/output-drain ownership into one `tokio::spawn` cleanup task, and discard partial output;
   - return the conventional result without awaiting that task;
   - the cleanup task sends TERM immediately, waits the fixed `JUDGE_CLEANUP_GRACE = Duration::from_millis(100)`, sends KILL if the group still lives, and reaps the child and drain tasks;
   - the child is also configured with `kill_on_drop(true)`, so runtime shutdown cannot intentionally orphan it.
5. Normalize and validate the conventional result exactly as today.
6. If a valid shadow observation exists, compare thresholded judge edges with normalized authoritative edges and emit a summary.
7. Return the conventional result or its existing error unchanged.

The shadow future never delays conventional completion. Judge completion also never short-circuits, retries, suppresses, or replaces conventional analysis.

When `use_llm_analysis = false`, the existing metadata-only result remains authoritative and the judge is skipped. The zero-change and single-change early returns remain unchanged and skip the judge.

## Process ownership

Create `src/judge_command.rs` rather than using `AiCommandRunner`:

- argv-safe `tokio::process::Command`, with no shell and no placeholder expansion;
- piped stdin/stdout/stderr;
- repository root as working directory;
- inherited process environment and current configured `envs`, but no provider-specific injection;
- isolated process group using the repository's existing process-group primitives;
- exact request write followed by stdin close;
- concurrent bounded stdout/stderr drains to prevent deadlock;
- timeout and explicit caller cancellation;
- synchronous TERM, fixed 100 ms grace, KILL, and mandatory reap when the judge's own timeout fires;
- ownership transfer to the detached cleanup task described above when conventional completion cancels a still-running judge;
- `kill_on_drop(true)` as shutdown backstop.

Do not reuse `AiCommandRunner`: its retry, stream-json textification, output callbacks, command queue, and long command timeouts are wrong for a bounded machine protocol.

No automatic retries are added. Provider-aware backoff remains the command's responsibility. This avoids duplicated calls and keeps shadow work bounded.

## Shadow comparison and observability

Represent the non-authoritative outcome internally:

```rust
struct DependencyJudgeObservation {
    model: String,
    values: HashMap<(String, String), f64>,
    usage: JudgeUsage,
    duration: Duration,
}
```

After conventional normalization, construct the authoritative semantic edge set from `AnalysisResult.dependencies`. Compare it with thresholded judge edges and count:

- matching positive edges;
- judge-only positive edges;
- analyzer-only positive edges;
- matching negative pairs;
- total pairs.

Emit one structured tracing event for success or failure. Allowed fields:

- purpose (`parallel_dependency`);
- outcome category;
- duration milliseconds;
- expected answer count;
- returned answer count when known;
- model identity only after schema validation;
- the five aggregate comparison counts.

Forbidden fields:

- proposal text or request state;
- API key or environment values;
- raw stdin, stdout, stderr, or provider error body;
- individual question instructions or answer values tied to named changes.

Failure categories are bounded enums such as `spawn`, `input_too_large`, `timeout`, `cancelled`, `nonzero_exit`, `output_too_large`, `invalid_json`, `schema_mismatch`, and `io`. Exit codes 2, 3, and 4 may be mapped to schema/input, authentication, and transient categories internally, but all remain non-authoritative shadow failures.

Judge state is process-local and ephemeral. It is not serialized, persisted, or used by `AnalysisProvenance`, retry debounce, blocker classification, dispatch, acceptance, or archive decisions, preserving the workspace-local workflow-state constitution.

## Test design

Use fake executables or an injectable process factory; do not require credentials or network access.

Configuration tests:

- absent config remains valid;
- valid argv/model/bounds load;
- empty argv/element/model, alias model, zero/excess timeout, invalid sizes, NaN/out-of-range threshold fail with actionable configuration errors;
- higher-priority complete entry replaces lower-priority entry atomically;
- omission inherits the lower-priority entry.

Wire tests:

- deterministic node ordering and question IDs;
- all and only permitted directed pairs are present;
- untrusted IDs cannot alter question keys;
- exact TypeSafe request fields and Noul criteria;
- configured pinned model is preserved;
- oversize input skips spawn.

Parser tests:

- compact and pretty valid responses;
- wrong model, missing/extra/wrong-type answers, duplicate/trailing JSON, prose, malformed JSON;
- NaN, infinity, negative, and greater-than-one values;
- invalid usage values;
- oversized stdout/stderr.

Runtime tests:

- conventional result is byte-for-byte structurally equal with config absent;
- valid judge result does not change conventional order/dependencies/provenance;
- explicit metadata dependencies survive regardless of judge value;
- spawn failure, exit 1/2/3/4, timeout, malformed output, and missing executable return the conventional result;
- a blocked/slow judge is transferred to cleanup without holding the conventional result, receives TERM, is killed after the fixed 100 ms grace when needed, and is reaped;
- a fast judge is retained for comparison while conventional analysis continues;
- conventional analyzer error is returned unchanged after the judge is cleaned up;
- zero, one-change, no-pair, and `use_llm_analysis = false` paths do not spawn the judge;
- logs expose only allowed aggregate fields.

## Rejected alternatives

### Generic Conflux envelope

Rejected because TypeSafe's request/response schema is accepted as the compatibility contract for later equivalent products. An adapter would add a second protocol without providing isolation.

### Sequential judge before conventional analysis

Rejected because timeout, startup, or provider latency would reduce Conflux availability even though the result is shadow-only.

### Sequential judge after conventional analysis

Rejected because it would delay analysis return solely for observability.

### Judge output as fallback or preferred authority

Rejected for this change. Calibration and false-positive dependency evidence do not yet exist. An authoritative mode requires a separate proposal with explicit routing semantics.

### Reusing `AiCommandRunner`

Rejected because an agent runner's shell templates, retries, textification, callback streaming, and long timeouts are incompatible with a bounded JSON machine protocol.

### Circuit breaker

Deferred. Concurrent cancellation ensures judge failure does not delay the conventional path. A circuit breaker adds mutable operational policy without being required for the first shadow measurement.

### Input sizing

The request grows quadratically in question count and linearly in included proposal bytes. The 1 MiB default is deliberately conservative: a larger queue may yield `input_too_large` and skip shadow collection while conventional analysis continues. Operators may raise the bounded limit up to 16 MiB after observing proposal sizes; Conflux never truncates proposal text or silently drops pairs.
