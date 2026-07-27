# Design: Runtime-owned Acceptance stalls

## Context

Acceptance-origin `APPLY_BLOCKED/marker.md` currently serves three roles at once: durable blocker evidence, restart routing, and explicit-retry token. Because it lives under the managed change directory, those control-plane roles mutate repository state. Workspace scanning then sees the generated marker before the Apply commit and reports `Blocked`, even though the implementation revision remains present.

Ordinary Acceptance retry context is already in-memory, and missing-verdict handling already provides a mode-independent bounded protocol retry. Reducer state can represent `stalled`, but it does not currently survive restart without the workspace marker. The new design must preserve restartability without letting hidden state become completion evidence.

## Constitutional Amendment

Amend the workspace-local workflow-state law with a narrow exception for temporary runtime pause/resume control. A versioned runtime record may suppress ordinary dispatch, restore a concrete stalled status, and select Acceptance as the retry phase only while it reconciles with current repository and worktree facts.

The exception does not permit runtime state to establish implementation completion, Acceptance PASS, archive readiness, merge eligibility, or base integration. Those outcomes remain repository/Git-derived under truthful completion. Deleting or corrupting runtime state changes only whether a concrete external hold remains displayed; it causes safe Acceptance re-execution for a complete unarchived Apply revision and never advances the workflow.

## Verdict Contract

Represent Acceptance outcomes as separate typed cases:

- canonical PASS, FAIL, and CONTINUE;
- bare blocker compatibility input requiring protocol correction;
- validated structured stalled blocker;
- missing verdict and existing command/permission/cancellation outcomes.

A structured blocker contains an explicit supported category, non-empty evidence, next action, and resumability. It may also include prerequisite owner and stable evidence identifiers. Runtime validates fields and repository-versus-external scope; it does not classify category by searching prose for `credential`, `token`, `auth`, or infrastructure words.

Bare `gated` and legacy `blocked` remain parser-compatible but are not sufficient stalled evidence. They enter the same fixed two-retry protocol driver used by missing verdict, with a GATED-specific corrective prompt. A canonical result resets the consecutive bare-GATED counter. Exhaustion returns a terminal protocol error and does not persist a stall.

## Runtime State Store

Use a dedicated Acceptance stall store in the existing XDG state hierarchy. The logical key is repository identity plus change ID; paths are attributes to validate, not sole identity.

The versioned record contains:

- schema version;
- repository identity;
- change ID;
- managed worktree identity and canonical path;
- branch when available;
- Apply revision;
- stalled phase;
- retry count;
- blocker category and concrete evidence;
- resumable flag;
- next action;
- optional prerequisite owner;
- created and updated timestamps.

Write through a temporary file and atomic rename. Corrupt or unsupported records are quarantined or reported and cannot control dispatch. The store must be injectable or path-parameterized for isolated tests.

## Reconciliation

Before a stored stall controls routing, verify:

1. repository identity matches;
2. change ID is active and not archived or merged;
3. the worktree belongs to that repository and has not been path-reused;
4. the stored Apply revision exists;
5. current HEAD equals or descends from the stored Apply revision;
6. repository evidence still identifies a complete unarchived Apply state.

A valid record restores `stalled` and suppresses ordinary dispatch. A mismatch invalidates or quarantines the record and returns to workspace-derived routing. If the Apply revision remains complete and unarchived, the safe route is Acceptance. No record permits direct archive entry.

## Explicit Retry

Explicit retry is a preparation transaction:

1. load and reconcile the stall record;
2. reject stale or non-resumable state;
3. reserve Acceptance retry intent against the same Apply revision;
4. start Acceptance through the normal configured command;
5. clear the prior hold only when dispatch has successfully crossed the preparation boundary.

If dispatch cannot start, retain the blocker record. Successful retry skips Apply. A repeated concrete blocker may update the same record; bare GATED during retry uses protocol budget and cannot recreate a stall by itself.

## Legacy Migration

Continue parsing legacy `acceptance-stalled-v1` markers only for migration. Migrate only when origin is Acceptance, resumability and blocker evidence are valid, and repository/worktree/Apply revision binding can be reconstructed. Then write runtime state first and remove only proven generated marker residue, verifying clean status afterward. Migration is idempotent.

Do not migrate or delete Apply-origin, unknown-origin, non-resumable, malformed, or ambiguously tracked markers. Their existing conservative blocked behavior remains until a separate Apply-side redesign.

## Lifecycle Isolation

Runtime stall state may influence:

- ordinary dispatch suppression;
- `stalled` operator status and blocker presentation;
- explicit retry eligibility and Acceptance resume phase;
- retry count and next-action display.

It must not influence:

- worktree clean/dirty evaluation;
- Apply commit existence or task completion;
- Acceptance PASS evidence;
- cleanup eligibility;
- archive artifact detection/readiness;
- merge eligibility or base integration;
- canonical OpenSpec contents.

## Alternatives Rejected

- Keep the Acceptance marker but ignore it for dirty checks: it still mixes runtime control with change artifacts and leaves migration/retry coupling.
- Treat bare GATED as stalled in memory only: restart loses a claimed concrete blocker, while evidence-free GATED remains incorrectly trusted.
- Persist PASS with the stall record: violates truthful completion and risks skipping Acceptance after restart.
- Infer blocker category from output text: produces unsupported credential/infrastructure classifications and cannot prove an external prerequisite.
- Remove all `APPLY_BLOCKED` handling: broadens scope into Apply/rejection handoffs that have separate lifecycle contracts.
