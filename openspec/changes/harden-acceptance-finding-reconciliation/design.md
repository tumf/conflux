# Design: Stable identity and monotonic acceptance completion

## Context

Acceptance findings cross three representations: reviewer output, runtime normalized findings, and runtime-owned checkbox tasks. Agent-authored prose is useful evidence but is not a stable state key. Skill instructions can reduce wording changes but cannot enforce workflow correctness.

## Decisions

### Runtime-owned identity

Runtime uses an explicit finding code when present. Otherwise it computes a deterministic identity from normalized structural fields such as repository scope, rule kind, and canonical repository location. Human-readable summary, evidence detail, line-number drift, and suggested remediation are mutable metadata rather than identity inputs.

The fallback algorithm must be deterministic across serial/parallel and process restarts. Collision fixtures must prove distinct rule/location combinations remain distinct. The implementation should reuse the normalized finding representation introduced by `compact-acceptance-retry-context` rather than introduce a second model.

### Monotonic completion outside FAIL ingestion

Runtime distinguishes two operations:

1. `merge_apply_progress`: hydrate or reconcile runtime findings while preserving completed status and agent-recorded evidence.
2. `replace_from_latest_fail`: treat the latest acceptance FAIL as the authoritative open set and reopen identities explicitly re-reported by that payload.

Only the second operation may transition completed to unchecked. This API boundary prevents call-site intent from being inferred from text or attempt history.

### Skill responsibility

Skills define producer behavior, not state correctness. `cflx-accept` produces atomic, current-state findings with stable codes when possible. `cflx-apply` does not rewrite runtime-owned finding text and records completion evidence separately. Runtime fallback and transition rules remain required even when agents violate or omit this guidance.

## Rejected Alternatives

- Exact finding text as identity: wording and evidence legitimately change during remediation.
- Skill-only enforcement: probabilistic instruction compliance cannot protect durable workflow state.
- Checkbox removal: it would discard repository-verifiable and human-readable implementation truth before checkpoint state is mature enough to replace it.
- Including line numbers in fallback identity: ordinary edits cause line drift and false new identities.

## Migration

The first update after deployment uses the dependency change's normalized current section. Legacy findings without explicit codes receive fallback identities during parsing. No out-of-worktree migration state is introduced.
