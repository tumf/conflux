# Design: Configurable operation skills

## Current Behavior

Runtime prompt builders hardcode operation-specific skill preludes:

| Operation | Current prelude | Main source |
|---|---|---|
| analyze | `load skills: cflx-analyze` | `src/orchestration/selection.rs` |
| apply | `load skills: cflx-apply` | `src/agent/prompt.rs` |
| rejecting review | `load skills: cflx-rejecting` | `src/orchestration/rejection.rs` |
| cleanup review | `load skills: cflx-cleanup-review` | `src/agent/prompt.rs` |
| accept | `load skills: cflx-accept` | `src/agent/prompt.rs` |
| archive | `load skills: cflx-archive` | `src/agent/prompt.rs` |
| resolve | `load skills: cflx-resolve` | `src/parallel/conflict.rs` |

These prompts otherwise carry variable context only. Fixed procedures stay in operation skills and command templates.

## Proposed Config Shape

Use explicit top-level optional keys rather than a nested dynamic map. This preserves the existing flat config style and makes template documentation straightforward.

```jsonc
{
  "analyze_skill": "cflx-analyze",
  "apply_skill": "cflx-apply",
  "rejecting_skill": "cflx-rejecting",
  "cleanup_review_skill": "cflx-cleanup-review",
  "accept_skill": "cflx-accept",
  "archive_skill": "cflx-archive",
  "resolve_skill": "cflx-resolve"
}
```

Each key is optional. Accessors return defaults when unset.

## Implementation Shape

1. Add optional fields to `OrchestratorConfig`.
2. Add defaults/accessors such as `get_apply_skill()` and `get_accept_skill()`.
3. Include all fields in config merge behavior.
4. Refactor prompt builders so the skill name is an argument or is resolved in the higher-level config-aware wrapper.
5. Update call sites:
   - `src/execution/apply.rs` / `src/agent/runner.rs` for apply/archive/acceptance paths
   - `src/orchestration/selection.rs` for analyze
   - `src/orchestration/rejection.rs` for rejecting review
   - `src/parallel/conflict.rs` for resolve prompts
   - cleanup-review call site for cleanup prompt
6. Update tests that currently assert fixed `load skills: cflx-*` strings so they cover both default and custom configured values.

## Backward Compatibility

No config changes are required for existing users. Missing keys return existing defaults.

The selected skill affects prompt text only. Existing command templates, stdout/stderr streaming, parser contracts, verdict markers, and workflow-control decisions remain unchanged.

## Validation and Safety

Because the value is interpolated into prompt text, implementation should either:

- validate a conservative skill identifier pattern such as `[A-Za-z0-9_.:/-]+`, or
- document that invalid/unloadable skill names fail at agent-runtime review time rather than config-load time.

Prefer at least rejecting empty or newline-containing values to avoid malformed prompt preludes.

## Verification Strategy

- Unit-test each accessor default.
- Unit-test merge precedence for at least one non-acceptance skill plus `accept_skill`.
- Unit-test prompt builders for default and custom skill names.
- Regression-test acceptance and resolve parser behavior to show only prelude selection changed.
- Verify config templates/docs mention the new keys.
