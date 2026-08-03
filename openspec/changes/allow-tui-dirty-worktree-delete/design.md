# Design: Explicit dirty worktree deletion in the TUI

## Decision

Use the service's fresh `Dirty` refusal to escalate into a typed destructive modal. Do not add dirty state to `WorktreeInfo` or remote DTOs. Keep known-dirty discard and teardown skipping as independent permissions.

## State machine

```text
D
  -> ConfirmWorktreeDelete(path, expected identity/ref)

Y
  -> DeleteIntent(skip_teardown=false, allow_known_dirty=false)

S
  -> DeleteIntent(skip_teardown=true, allow_known_dirty=false)

service result:
  clean + eligible -> delete
  Dirty -> ConfirmDirtyDiscard(path, identity, branch, head, skip_teardown)
  anything else -> retain + actionable warning

ConfirmDirtyDiscard:
  uppercase X -> DeleteIntent(captured skip_teardown, allow_known_dirty=true)
  N/Esc -> cancel
  every other key -> no mutation
```

The existing ordinary modal must display both `Y` and `S` semantics. The destructive modal states permanent loss of tracked/index changes, reported non-ignored untracked entries, and possible generated/ignored directory content.

## Mutation boundary

```text
shared repository-scoped guard
  -> fresh observation
  -> expected Git identity, branch and HEAD/ref validation
  -> main/base-merge/ahead/dirty safety checks
  -> teardown unless skipped
  -> second observation
  -> repeat identity/ref and non-dirty safety checks
  -> require Dirty or Clean according to explicit policy; Unknown always refuses
  -> structured dirty-discard warning
  -> git worktree remove --force
  -> delete branch only when ref/reachability still match validated facts
  -> events/refresh
```

TUI and Web receive the same `Arc<WorktreeService>` or an equivalent shared repository-scoped guard. A per-command service-local mutex is insufficient.

Active/deleting eligibility currently belongs to TUI state rather than `WorktreeFacts`. The TUI rechecks it before dispatch. If implementation cannot make activation and deletion share a reservation boundary, the guarantee is limited to latest TUI state at dispatch plus repository facts at service mutation; it must not claim atomic exclusion against later activation. A process-local delete reservation may be introduced only if needed to close that race and must be honored by run/queue admission.

## Observation semantics

Known dirty is based on explicit porcelain status with non-ignored untracked reporting. Ignored-only content can appear clean. Safety-critical failures for dirty, base resolution, commits ahead, base merge, identity, or ref state are explicit unknown/errors and refuse deletion.

Known dirty-discard waives only known `Dirty`. It never waives an unknown fact, commits ahead, main status, or identity/ref mismatch.

## Branch preservation

Worktree removal and branch deletion are distinct outcomes. After forced worktree removal, branch cleanup remains best-effort but may use destructive deletion only when the branch ref equals the validated OID and reachability can be reconfirmed. A moved or unverifiable ref is retained with warning so commits do not become unreachable through a stale cleanup decision.

## Threat boundary

The shared guard serializes Conflux-owned mutations. Re-observation detects observable drift before removal. External Git processes can still race after the final check; no filesystem/Git atomic transaction is claimed.

## Alternatives rejected

- Current `Y` as force: insufficient data-loss distinction.
- `S` as force: conflates teardown and dirty permissions.
- TUI dirty projection: stale and unnecessarily widens DTO/state.
- Auto stash/commit: creates unrequested repository state.
- Remote force: violates the closed remote contract.
- Dirty fingerprints or ignored-file scans: unnecessary for explicit disposable deletion.

## Verification strategy

Pure tests cover policy and unknown states. TUI tests cover the exact key matrix and captured teardown bit. Service tests cover shared serialization and both observations. Real-Git heavy tests mutate identity/ref and teardown state and prove branch retention. API/OpenAPI tests prove the remote capability remains absent. Each filtered suite first uses `--list` plus a non-empty assertion so absent new tests cannot pass silently.
