# Design: External Acceptance evidence cache

## Authority boundary

Repository files and Git state remain workflow authority. External Acceptance state is cache and diagnostics only.

- Repository/Git state determines candidate identity, review range, proposal declarations, cleanliness, archive readiness, and merge eligibility.
- Live typed reducer state may suppress an unchanged retry within one owner lifetime.
- External manifests, evidence envelopes, and artifacts may avoid rerunning an exact successful gate but may not suppress work, imply PASS, or choose the next workflow action.
- Owner restart loses process-local retry suppression. An explicit retry receives one new bounded attempt.

## Storage layout

Resolve one root from the existing Conflux state-root policy:

```text
<conflux-state-root>/acceptance/
  <project-slug>/
    <workspace-slug>/
      <change-id>/
        manifest.json
        gates/
          <verification-id>.json
          <verification-id>.log
```

`project-slug` and `workspace-slug` are generated from canonical absolute paths with the existing slug algorithm. `change-id` and `verification-id` retain strict path-component validation. No resolved path may be within the target repository or any managed worktree.

When no persistent root exists, use a Conflux-owned system temporary root with the same isolation. If root creation, permission validation, or writes fail, return a typed Acceptance runtime hold before launching a gate. Never fall back to the target tree.

## Lifecycle

1. Resolve and validate the external store before gate execution.
2. Build the current manifest from repository state in memory.
3. Read cached evidence only for exact reuse comparison.
4. Execute missing or invalid gates in the unchanged target working directory while storing output externally.
5. Recheck the full target status without excluded paths.
6. Persist cache/diagnostics externally.
7. Clear or retain external cache according to existing settlement needs; either choice must not change workflow authority.

## Legacy handling

The runtime does not read, migrate, rewrite, hide, or delete target-local `.cflx/verification-evidence` content. Full cleanliness treats it like any other target content, including ignored content when the relevant cleanliness contract requires ignored-path observation. The diagnostic identifies the path and requires explicit repository-owner cleanup.

## Retry semantics

The prior durable terminal-manifest guard is removed. Same-owner suppression uses typed reducer state already projected for the non-resumable hold. Restart cannot reconstruct refusal from external cache. This deliberately permits one new bounded explicit attempt after restart while retaining valid gate-result reuse.
