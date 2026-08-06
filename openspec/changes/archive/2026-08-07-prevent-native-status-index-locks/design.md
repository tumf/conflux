# Design: Native read-only Git status lock policy

## Context

Conflux already suppresses optional index writes for one periodic change-list query, but status execution remains distributed across shared Git helpers, conflict-resolution context capture, Apply/Archive state modules, merge helpers, and the upstream adapter. These paths differ in output shape and error type, so replacing every call with one high-level boolean helper would lose required semantics.

The incident confirms the missing invariant rather than a need for a new scheduler lock: a Conflux-owned status process was observed holding the root index lock while lifecycle work needed the same index. The lock can appear after `on_merged` preflight, so preflight waiting alone cannot exclude this race.

## Decision

### Child-local Git global option

Every production native read-only status invocation uses this command shape:

```text
git --no-optional-locks status <existing status arguments>
```

`--no-optional-locks` is a Git global option and must precede `status`. The implementation must not set `GIT_OPTIONAL_LOCKS` on the Conflux process or a shared command runner because that would also affect mutating descendants.

### Shared policy, preserved adapters

Use the smallest shared command-construction primitive that can preserve all current contracts. Shared helpers should reuse one argv prefix or builder. Direct execution modules and `GitUpstreamOps`, which have module-specific captured-output and error contracts, may keep their adapters but must consume or assert the same command policy.

Do not collapse these distinct observations:

- trimmed boolean dirty/clean status;
- human-readable plain status text captured for conflict-resolution prompt context;
- untrimmed porcelain bytes used by the Apply stage gate;
- explicit untracked and ignored modes;
- pathspec-scoped residue status;
- porcelain v2 used for structural upstream classification.

### Production inventory boundary

The invariant covers native `git status` commands constructed by Conflux production code, whether they classify state or capture human-readable context. It does not rewrite agent commands, user hooks, arbitrary configured shell commands, or test fixture commands. The inventory regression must inspect production argv construction or shared-policy use rather than scan all source text, where test fixtures, display strings, diagnostics, and prompt prose legitimately mention `git status`.

The invariant is status-specific. Worktree-scoped `git diff` may also refresh index stat data opportunistically, but this policy does not assume Git gates that path behind `--no-optional-locks`; diff-path contention requires a separately scoped mechanism and verification.

## Verification Strategy

1. Exact argv unit tests assert global-option ordering for every command-construction adapter.
2. Existing classification fixtures prove no semantic drift.
3. A temporary-repository positive control first proves plain status can change complete index bytes for the fixture.
4. The same restored fixture is observed through representative production paths; returned status must be current while index bytes remain identical.
5. Upstream command recording proves porcelain v2 is preserved and non-status mutation argv is unchanged.

Each new default-suite test must remain below one second. If a real-Git matrix cannot do so after fixture reuse, mark only that integration test heavy under repository policy; keep command-shape unit coverage in the default suite.

## Alternatives Rejected

### Extend only `on_merged` preflight waiting

A status poll can acquire an optional lock after preflight returns and while the hook runs. Waiting longer does not reserve the index and does not remove self-contention.

### Set `GIT_OPTIONAL_LOCKS=0` for the Conflux process

This is broader than required and may alter mutating Git operations or user-configured descendants. Child argv provides the native narrow control.

### Serialize every Git operation

A process mutex would not coordinate a second Conflux process or external Git client, and serializing read-only queries behind lifecycle work adds complexity when Git already provides the exact read-only option.

### Retry or remove the lock

Retry requires operation-specific identity and ambiguous-success proof. Removing a lock cannot safely establish ownership. Neither is needed to stop Conflux's own observations from requesting optional writes.
