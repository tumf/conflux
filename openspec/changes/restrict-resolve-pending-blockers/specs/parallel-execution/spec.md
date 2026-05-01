## MODIFIED Requirements

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, the orchestrator must attempt to merge or defer the change according to structured blocker classification rather than leaving auto-resumable and manual-wait cases ambiguous.

A deferred merge caused by another active non-terminal change in `Resolving` or `Rejecting` SHALL advance into reducer-owned auto-resumable handling (`ResolveWait` or immediate resolving when promoted).

A deferred merge caused by active `Applying`, `Accepting`, `Archiving`, terminal `Rejected`, dirty base, or other manual intervention requirement SHALL NOT be classified as automatic `ResolveWait` solely because that state exists. Dirty base and manual intervention deferrals SHALL remain in manual merge wait handling (`MergeWait`).

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

#### Scenario: active applying deferred archive does not promote to resolve wait

**Given**: a change is archived in parallel mode
**And**: another change is actively `Applying`
**When**: post-archive merge handling is evaluated
**Then**: the archived change is not classified as auto-resumable `ResolveWait` because of that applying change

#### Scenario: active accepting deferred archive does not promote to resolve wait

**Given**: a change is archived in parallel mode
**And**: another change is actively `Accepting`
**When**: post-archive merge handling is evaluated
**Then**: the archived change is not classified as auto-resumable `ResolveWait` because of that accepting change

#### Scenario: dirty-base deferred archive stays merge wait

**Given**: a change is archived in parallel mode
**And**: merge is deferred because the base branch is dirty while no other change is actively `Resolving` or `Rejecting`
**When**: the deferred merge result is processed
**Then**: the change remains in manual merge wait handling (`MergeWait`)
**And**: it is not classified as auto-resumable
