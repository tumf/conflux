---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/observability/spec.md
  - scripts/cflx-log-mine.py
---

# Fix log mining streaming scalability

**Change Type**: implementation

## Problem / Context

`cflx-log-mine.py` is the bundled helper for inspecting Conflux runtime logs after a marker timestamp. It currently reads each selected log file fully into memory before scanning. Large or long-lived log files can make the helper exceed practical interactive time limits before it prints any report, which prevents operators from reliably checking error groups, manual resolve markers, and resolve/merge timelines.

The helper is observability-only. Any fix must not introduce durable workflow-control state or use mined log output to decide scheduling, acceptance, archive, merge, or resume routing.

## Proposed Solution

Refactor the bundled log mining helper to scan selected log files incrementally while preserving its existing report shape and redaction behavior.

- Stream each log file line-by-line instead of loading the entire file into memory.
- Keep bounded context for examples and marker hits without retaining entire files.
- Preserve `--change-id`, `--format text|json`, `--top`, `--max-examples`, and `--context` semantics.
- Ensure sensitive absolute paths and volatile identifiers continue to be normalized in grouped output.
- Add regression coverage using generated large log fixtures so the helper must produce output without whole-file buffering.

## Acceptance Criteria

- Running `python3 scripts/cflx-log-mine.py --top 30` on a log root containing a large selected log completes and emits the report header, top groups, manual operation markers, action timeline markers, and follow-up query hints.
- The helper does not require loading an entire log file into memory to produce grouped errors or marker examples.
- `--change-id <id>` filters examples and timelines consistently with the existing behavior.
- Text and JSON output remain schema-compatible for existing consumers.
- Redaction/normalization prevents repository-specific absolute paths and volatile ids from appearing in grouped keys.
- The implementation remains observability-only and does not affect runtime workflow decisions.

## Explicit Completion Conditions

This change is complete when repository evidence shows:

- `scripts/cflx-log-mine.py` uses an incremental scanner with bounded per-hit context storage rather than `Path.read_text().splitlines()` over every selected file.
- Tests or script-level fixtures cover large-log scanning, manual/action marker extraction, `--change-id` filtering, and JSON output compatibility.
- Verification commands include a direct invocation against a generated temporary log root with a marker file and a large selected log.
- `cflx openspec validate fix-log-mine-streaming --strict --evidence warn` passes.

## Completeness Checklist

- User-facing outcome: operators can reliably run the bundled log mining helper before analyzing Conflux errors.
- Repository areas likely requiring change: `scripts/cflx-log-mine.py` and its test coverage or script fixture tests.
- Required verification: unit/integration-style generated fixture tests plus direct CLI invocation.
- Dependencies and rollout: no migration and no durable state changes.
- Non-goal: do not change Conflux runtime scheduling, merge, retry, acceptance, archive, or workflow-control behavior based on mined logs.

## Out of Scope

- Changing log retention policy.
- Adding external dependencies to the helper.
- Mining or storing confidential log contents in repository artifacts.
- Modifying Conflux runtime behavior based on helper output.
