## Context

Acceptance has three distinct responsibilities:

1. The Acceptance agent observes repository state and returns a verdict with evidence.
2. Conflux runtime parses the verdict and persists the latest repository-fixable findings.
3. Apply consumes the runtime-owned follow-up and repairs the repository.

The portable skill and canonical spec already enforce this boundary. `src/config/defaults.rs` invokes the tracked OpenCode command by default, and `.opencode/commands/link.sh` exposes it through a symlink, so no generated or installed copy sits between the tracked adapter and execution. Only that adapter still assigns persistence to the reviewer.

## Decision

### Keep the reviewer read-only

Replace the adapter's post-FAIL mutation instructions with an explicit prohibition on editing `tasks.md` or runtime-owned follow-up state. A reviewer reports findings only.

This prevents the review process from invalidating its own clean-tree observation and keeps Acceptance evidence distinct from Apply mutations.

### Keep runtime as the sole follow-up writer

The adapter will name Conflux runtime as the owner of normalized `## Current Acceptance Follow-up` persistence. It will not reproduce rendering, numbering, deduplication, reopening, or external-blocker rules already implemented in runtime.

### Add a narrow adapter drift test

`src/embedded_skills.rs` already embeds `.opencode/commands/cflx-accept.md` under `#[cfg(test)]` and tests other prompt ownership guarantees. Extend that boundary with one test that:

- requires explicit read-only and runtime-persistence language;
- forbids exactly `After listing all findings, update openspec/changes/<change_id>/tasks.md`;
- forbids exactly `Determine the next acceptance attempt number`;
- forbids exactly `Append or create the section for that attempt`.

The test deliberately does not forbid broad substrings such as `update tasks.md` or the numbered follow-up heading because legitimate prohibition text may name them. It verifies the actual tracked adapter consumed by OpenCode rather than a duplicate fixture.

## Alternatives Rejected

### Let the reviewer edit and exempt its mutation from clean-tree checks

Rejected because it makes a review phase mutate the evidence it judges and conflicts with the canonical read-only contract.

### Generate every runtime adapter from the portable skill

Rejected as unnecessary for this focused repair. The adapter has runtime-specific command syntax and a small contract test is sufficient to prevent recurrence.

### Change runtime persistence again

Rejected because runtime already writes one latest-only `## Current Acceptance Follow-up`; the defect is stale adapter guidance, not missing persistence behavior.

## Compatibility and Risk

The change preserves accepted JSON and legacy verdict forms and does not alter runtime parsing. Agents that already followed the portable skill see no behavior change. OpenCode reviewers stop producing their own numbered sections and rely on existing runtime handling.

String-level prompt assertions can be brittle, so the test checks explicit positive ownership language and the three pre-change imperative anchors rather than snapshotting the whole adapter or matching broad words that can also appear in prohibitions. A reviewer verifies those assertions and the passing test from the current tree; it does not mutate the adapter to recreate the stale tail.
