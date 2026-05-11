# Design: Permission Denial as Stalled Execution Blocker

## Context

Conflux has separate concepts for dependency `blocked`, resumable execution `stalled`, terminal `error`, and normal acceptance failure retry. Permission and local policy denial belongs to the resumable execution hold lane because the workspace can be preserved and retried after an operator changes local permissions, but the agent usually cannot fix the denial by editing repository code.

The existing apply implementation already detects `permission requested` + `auto-reject`, but currently logs it as a soft error and continues. Acceptance has `Gated`/`AcceptanceGated` support for infrastructure-style holds, but permission denial can still pass through command-failure or FAIL-finding paths.

## Constraints

- Workflow routing must remain derivable from workspace file state, workspace git state, and base-branch tree comparison.
- External logs, metrics, UI state, and in-memory metadata are allowed for observability but not as authoritative workflow-control inputs.
- Dependency `blocked` must remain reserved for queue-side unresolved dependency conditions.
- Normal implementation failures must remain retryable so acceptance can drive follow-up apply work.
- First-observed or progressing permission/policy denials may be transient and must not immediately become stalled.
- Repeated unresolved permission/policy denial should be resumable after operator action, not terminal.

## Approach

### Shared classifier

Introduce or extend a shared classifier around `src/permission.rs` that accepts one or more text sources:

- stdout tail
- stderr tail
- command error string
- acceptance findings

It should return structured information such as category, denied target when known, phase supplied by caller, denial signature, and operator guidance. The classifier should match explicit harness/tool permission denial patterns and avoid broad matching that would incorrectly convert ordinary implementation failures into stalled holds.

### Repetition and progress gate

A classified denial becomes stalled only when the same unresolved denial signature recurs without repository-visible progress. The signature should be stable enough to identify repeated denial of the same required target or operation, while avoiding false equivalence between unrelated permission prompts.

Progress evidence should be based on workspace-observable facts already available to the runtime, such as task progress, tracked/untracked workspace file changes, WIP revision changes, or materially changed acceptance evidence. In-memory counters may be used while a run is active, but they must not become authoritative durable routing state outside the workspace.

### Apply path

When apply detects a classified denial after an agent command exits or appears in captured output, apply should compare it with the prior denial/progress context. The first denial, a changed denial signature, or denial accompanied by repository-visible progress may continue through existing retry behavior. A repeated same-signature denial with no repository-visible progress should stop immediately. It should not continue to another apply iteration, post-process the denial as empty WIP, or rely on repeated stall detection. The parallel executor should translate the repeated unresolved denial into a reducer-observable stalled state and preserve the workspace.

### Acceptance path

Acceptance dispatch should classify command failures and FAIL findings before existing terminal-error or retry handling:

1. If command failure text/findings classify as permission/policy denial for the first time or with changed progress/evidence, keep the existing non-stalled result path.
2. If command failure text/findings classify as repeated unresolved permission/policy denial, emit/record stalled hold and return without terminal error.
3. If FAIL findings classify as repeated unresolved permission/policy denial, skip ordinary follow-up task persistence and return stalled hold.
4. Otherwise keep current behavior: command failures remain terminal errors, and normal FAIL findings persist follow-up tasks then return to apply.

### Event and reducer path

Prefer a general blocker event over overloading `ProcessingError`:

- `ExecutionEvent::ExecutionBlocked { change_id, blocker }`, or an equivalent generalized stalled event.
- Reducer sets `activity = Idle`, `wait_state = Stalled`, `terminal = None`, and metadata describing permission/operator remediation.

If the implementation reuses existing `WorkspaceStatusUpdated { status: Blocked }`, it must still ensure the reducer produces `stalled` display status and avoids dependency-blocked wording.

## Trade-offs

A dedicated blocker event is a larger API change than string-classifying errors in `ProcessingError`, but it avoids mixing self-heal-impossible permission policy failures with terminal errors and keeps UI state semantics clearer. Classifier patterns should stay conservative to avoid masking real implementation failures.
