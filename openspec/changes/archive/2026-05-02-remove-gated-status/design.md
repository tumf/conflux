# Design: Simplify blocker-adjacent lifecycle states

## State model

The user-facing lifecycle taxonomy should keep only one resumable intervention hold state: `stalled`.

- `blocked`: dependency wait before work can proceed.
- `stalled`: non-terminal hold requiring review, user input, external setup, or a non-automatic recovery decision.
- `rejecting`: active rejection-proposal review is running.
- `rejected`: terminal rejection confirmed.

This removes `gated` as an end-user state while preserving the reason that produced the hold in metadata.

## Routing semantics

Acceptance may still parse `gated` verdicts for compatibility, but that verdict no longer maps to a distinct display status. Instead:

1. Acceptance blocker is observed.
2. Runtime records a stalled hold or enters `rejecting` if there is a rejection proposal to review.
3. Blocker metadata records categories such as `acceptance-gated`, `rejection-review-block`, or `external-dependency`.
4. Confirmed rejection alone transitions to terminal `rejected`.

## Compatibility

Parser compatibility is intentionally separated from lifecycle taxonomy. `gated` may remain a recognized acceptance verdict token while no UI/API lifecycle surface emits `gated` as a status.

## Constitution alignment

The design does not introduce external durable workflow state. Resume/routing decisions remain derived from workspace file state, workspace git state, and base-branch comparison, with metadata used only when it is part of reducer state derived from those inputs/events and not an external authority.
