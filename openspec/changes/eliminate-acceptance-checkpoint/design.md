# Design: Acceptance Without a JSON Checkpoint

## Decision

Do not persist ordinary acceptance lifecycle state in a generated JSON file. Acceptance is an operation that must run before archive, not a durable artifact whose previous PASS should be trusted after interruption.

## Runtime State

Within one orchestration process, existing runtime structures may carry:

- current acceptance status;
- revision under review;
- cycle count;
- normalized previous findings;
- semantic progress baseline.

This context is discarded on process restart.

## Durable Evidence

Only evidence with an independent repository purpose remains durable:

- `tasks.md` records concrete acceptance repair findings;
- `APPLY_BLOCKED/marker.md` records a resumable stalled hold after retry safeguards decide further automatic work must stop;
- archived OpenSpec paths, Git state, and base-branch tree state prove archive and merge progress.

No replacement hidden checkpoint, external database, cache, or commit metadata is introduced.

## Resume Routing

- Incomplete implementation tasks route to apply.
- Complete but unarchived implementation routes to acceptance.
- An interrupted archive that is not repository-verifiably complete routes through acceptance again before archive finalization.
- A valid archive entry with the active change removed may continue to post-archive resolve/merge handling.
- Base-integrated evidence remains terminal/idempotent evidence.
- A resumable acceptance-generated blocker marker remains blocked until explicit retry consumes it.

Re-running acceptance is deliberate. It trades duplicate work after interruption for fewer hidden states and truthful archive gating.

## Retry Semantics Across Restart

Before a stalled marker is committed, retry count and semantic baseline are process-local. Restart begins a fresh acceptance sequence. This can permit additional automatic attempts across repeated restarts, but cannot cause unverified archive or data loss.

Once the configured active-run safeguard is exhausted or semantic no-progress triggers a stalled hold, the tracked blocker marker remains the durable stop condition. Explicit retry retains existing marker-origin and resumability checks.

## Archive and Merge

Archive code no longer deletes acceptance JSON before staging or after merge. Therefore checkpoint cleanup cannot create a tracked deletion or false dirty worktree. Existing checks remain authoritative for:

- unrelated Git changes;
- active change directory presence;
- valid archive entry presence and layout;
- base repository safety;
- merge lane availability.

## Module Boundary

`src/parallel/acceptance_state.rs` currently combines JSON checkpoint state and blocked-marker behavior. Implementation should delete checkpoint responsibilities while retaining or minimally relocating blocked-marker behavior. No new abstraction is required unless deletion leaves the module name actively misleading.

## Verification

Temporary repositories and real dispatch tests must prove behavior, not only absence of symbols. The primary regression starts without `.cflx/acceptance-state.json`, runs through acceptance PASS and archive, and verifies the path never appears and post-archive routing does not emit a manual deferral.
