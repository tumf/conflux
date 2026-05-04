## MODIFIED Requirements

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, the orchestrator must attempt to merge or defer the change according to structured blocker classification rather than leaving auto-resumable and manual-wait cases ambiguous.

A deferred merge caused by another active non-terminal change in `Resolving` or `Rejecting` SHALL advance into reducer-owned auto-resumable merge/resolve handling (`ResolveWait` or immediate resolving when promoted). Active `Rejecting` is included because rejection review can touch and dirty base state.

A deferred merge caused by active `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, dirty base without an active base-mutating lane occupant, or other manual intervention requirement SHALL NOT be classified as automatic `ResolveWait` solely because that state exists. Dirty base and manual intervention deferrals SHALL remain in manual merge wait handling (`MergeWait`).

The implementation MUST NOT infer auto-resumable versus manual-wait behavior by parsing a human-readable deferred reason string.

#### Scenario: active resolving deferred archive promotes to resolve wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because another change is actively `Resolving`
**When**: the deferred merge result is processed
**Then**: the archived change enters auto-resumable deferred handling (`ResolveWait` or equivalent queued resolve intent)
**And**: this decision does not depend on parsing a free-form reason string

#### Scenario: active rejecting deferred archive promotes to resolve wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because another change is actively `Rejecting`
**When**: the deferred merge result is processed
**Then**: the archived change enters auto-resumable deferred handling (`ResolveWait` or equivalent queued resolve intent)
**And**: rejection review completion or failure triggers retry of deferred merge work

#### Scenario: dirty-base deferred archive stays merge wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because the base branch is dirty while no other change is actively `Resolving` or `Rejecting`
**When**: the deferred merge result is processed
**Then**: the change remains in manual merge wait handling (`MergeWait`)
**And**: it is not classified as auto-resumable

### Requirement: Parallel rejecting lane dispatch

Parallel execution SHALL route rejection-review handoff through the same single base-mutating lane used by merge/resolve operations. A change that needs rejection review may enter active `Rejecting` only when no other non-terminal change is actively `Resolving` or `Rejecting`.

If the base-mutating lane is occupied, the rejection-review handoff SHALL become reducer-owned `RejectWait` and display `reject pending`. This wait is auto-resumable and MUST NOT require manual user action.

#### Scenario: rejecting handoff waits behind resolving

**Given**: Change A is actively `Resolving`
**And**: Change B apply execution records `openspec/changes/<change_id>/REJECTED.md`
**When**: parallel dispatch handles B's rejecting handoff
**Then**: B does not start rejection review immediately
**And**: B enters `RejectWait`
**And**: B displays `reject pending`
**And**: B is retried automatically after A clears the base-mutating lane

#### Scenario: rejecting handoff waits behind rejecting

**Given**: Change A is actively `Rejecting`
**And**: Change B apply execution records `openspec/changes/<change_id>/REJECTED.md`
**When**: parallel dispatch handles B's rejecting handoff
**Then**: B does not start rejection review immediately
**And**: B enters `RejectWait`
**And**: B displays `reject pending`
**And**: B is retried automatically after A's rejection review completes or fails

#### Scenario: rejecting handoff starts when lane is clear

**Given**: no non-terminal change is actively `Resolving` or `Rejecting`
**And**: Change B apply execution records `openspec/changes/<change_id>/REJECTED.md`
**When**: parallel dispatch handles B's rejecting handoff
**Then**: B enters active `Rejecting`
**And**: B displays `rejecting`
**And**: no other change is active in the base-mutating lane
