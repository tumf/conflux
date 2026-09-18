# Design: isolated judge evaluation records

## Authority boundary

Evaluation data is disposable observability, not workflow state. Writers run after or alongside shadow collection, and all errors are swallowed after one bounded warning. No reader is reachable from analysis normalization, provenance, queue reduction, dispatch, blocker classification, Acceptance, Archive, or lifecycle decisions. Removing the complete evaluation root cannot change the next action for unchanged workspace and Git state.

## Storage layout

The existing state-root resolver remains authoritative:

```text
<state-root>/
  logs/                                      # existing operational logs
  evaluations/
    .pair-id-salt                            # 32 random bytes, mode 0600
    parallel_dependency/
      <project-slug>/
        <YYYY-MM-DD>.jsonl                   # mode 0600
```

`evaluations` and descendants use mode `0700` where supported. No evaluation path is returned from `/api/v2/logs`, `/api/v2/events`, `/api/v2/ws`, the TUI log ring, or `cflx logs`.

One daily project file is the minimum implementation. Each record is serialized to a single byte buffer ending in `\n`, opened append-only, locked for the append where the platform supports the repository's existing file-lock abstraction, written in one critical section, flushed, and unlocked. A failed append leaves orchestration unchanged and produces no retry loop.

## Configuration contract

The existing atomically replaced purpose entry gains:

```jsonc
{
  "judge_commands": {
    "parallel_dependency": {
      "command": ["jev", "run", "-"],
      "model": "jev-1.13.0",
      "mode": "shadow",
      "evaluation": {
        "enabled": true,
        "retention_days": 30,
        "max_total_bytes": 104857600
      }
    }
  }
}
```

Rules:

- `evaluation` omitted means disabled.
- `enabled: false` does not create directories, salt, or records; optional bounds, if present, are validated but unused.
- `enabled: true` requires positive `retention_days` and `max_total_bytes`.
- Maximums are 3650 days and 10 GiB. Values above those bounds fail configuration loading.
- The complete higher-priority `parallel_dependency` entry replaces lower-priority command, model, mode, limits, threshold, and evaluation settings together.
- This change accepts only `mode: "shadow"`; absent mode normalizes to shadow for backward compatibility. Other modes remain invalid until separately specified.

## Record schema

All records share:

```json
{
  "schema_version": 1,
  "recorded_at": "RFC3339 UTC",
  "conflux_version": "0.6.325",
  "purpose": "parallel_dependency",
  "mode": "shadow",
  "outcome": "observed|spawn|input_too_large|timeout|cancelled|nonzero_exit|invalid_request|authentication|transient|output_too_large|invalid_json|schema_mismatch|io",
  "duration_ms": 892,
  "expected_answers": 2
}
```

An `observed` record additionally requires:

```json
{
  "model": "jev-1.13.0",
  "returned_answers": 2,
  "input_tokens": 1038,
  "output_tokens": 56,
  "yes_threshold": 0.8,
  "pairs": [
    {
      "pair_id": "sha256:<64 lowercase hex>",
      "judge_probability": 0.97,
      "judge_dependency": true,
      "analyzer_dependency": true
    }
  ]
}
```

Failure records use the existing `JudgeFailureCategory::as_str()` token verbatim. The schema-v1 mapping is exhaustive: `Spawn` → `spawn`, `InputTooLarge` → `input_too_large`, `Timeout` → `timeout`, `Cancelled` → `cancelled`, `NonzeroExit` → `nonzero_exit`, `InvalidRequest` → `invalid_request`, `Authentication` → `authentication`, `Transient` → `transient`, `OutputTooLarge` → `output_too_large`, `InvalidJson` → `invalid_json`, `SchemaMismatch` → `schema_mismatch`, and `Io` → `io`. Failure records omit model and pairs unless a model was already validated; they never include stderr or provider bodies. Unknown future schema versions and malformed lines are not counted as valid observations.

## Pair pseudonymization

Pair identity input is unambiguous binary framing:

```text
"cflx-judge-eval-v1\0"
len(project_slug) || project_slug
len(dependent_id) || dependent_id
len(dependency_id) || dependency_id
```

Lengths are unsigned big-endian 64-bit integers. The digest is SHA-256 over `salt || framed_input`; the public value is `sha256:` plus lowercase hex. This is local pseudonymization, not an authentication primitive. The random installation salt prevents unsalted dictionary comparison across installations. Declare `sha2 = "0.10"` directly rather than relying on either transitive lockfile version.

Salt creation writes one uniquely named temporary file with create-new, fsyncs it, then atomically installs it without replacing an already installed salt. A loser in a concurrent first-writer race discards its temporary file and reads the installed salt. Existing salt must be exactly 32 bytes, a regular file, not a symlink, and mode `0600` where supported. Corruption fails only the evaluation write.

## Cleanup contract

Cleanup runs at owner startup only when evaluation is enabled, plus before opening the current daily file if startup did not run in the current command path. No daemon is added.

1. Canonicalize the evaluation root without following descendant symlinks.
2. Inventory only recognized regular files matching `parallel_dependency/<project-slug>/YYYY-MM-DD.jsonl`.
3. Protect the selected current writer path.
4. Remove files older than `retention_days`.
5. Recompute bytes, then remove files across all projects in ascending filename date until total regular JSONL bytes are at or below `max_total_bytes`; tie-break by canonical relative path.
6. Never remove `.pair-id-salt`, unknown files, directories, symlinks, or ordinary logs.
7. Return a report containing considered, removed, freed bytes, protected files, and failures. Partial failure does not roll back successful removals or affect orchestration.

The current daily file may temporarily exceed the global budget; active-file protection wins. The next day or explicit future purge feature may evict it after it is no longer active.

## Summary command

Command shape:

```text
cflx judge-eval summary --purpose parallel_dependency [--since-days N] [--project SLUG] [--json]
```

Defaults:

- `--since-days` defaults to the configured retention period when available, otherwise 30.
- no project filter scans recognized project directories under the dedicated purpose root.
- human output is concise; `--json` emits exactly one versioned object.

Metrics:

- files and lines examined
- valid observed and failure records
- malformed/unsupported lines skipped
- outcome counts and rates
- TP, FP, FN, TN from pair labels
- precision, recall, specificity as nullable values when denominators are zero
- p50 and p95 duration using deterministic nearest-rank percentiles
- input/output token totals
- grouped counts by validated model and Conflux version

The summary command is read-only. It does not create the root or salt, initialize tracing files, run cleanup, or modify access-independent repository state.

## Failure handling

- Configuration defects fail startup before orchestration.
- Runtime storage, salt, lock, append, cleanup, and summary read errors are typed locally.
- Recorder/cleanup errors log one bounded path-free warning per attempted operation.
- Summary returns non-zero only for root-level selection/read failures; malformed individual lines are counted and skipped.
- No raw record is copied into ordinary logs.

## Verification design

Focused tests use temporary state roots and deterministic injected clock/salt. They prove:

- disabled mode creates nothing;
- observed and every failure class serialize to one schema line;
- raw IDs, proposal text, paths, commands, environment values, and raw process output are absent;
- pair IDs are stable for one salt, directional, project-scoped, and differ across salts;
- concurrent writers produce parseable complete lines;
- corrupt/symlink salt and unwritable roots degrade without changing analysis;
- age and byte retention are independent, global, deterministic, symlink-safe, and active-file-safe;
- ordinary log cleanup cannot reach evaluations and evaluation cleanup cannot reach logs;
- summary metrics and percentiles are exact for fixtures, malformed/future records are counted but excluded;
- summary does not create or mutate state;
- deleting the evaluation root leaves the authoritative analysis result identical.
