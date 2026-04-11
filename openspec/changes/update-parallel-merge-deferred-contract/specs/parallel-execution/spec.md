## MODIFIED Requirements

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, the orchestrator must attempt to merge or defer the change according to the canonical merge-deferred contract rather than leaving auto-resumable and manual-wait cases ambiguous.

A deferred merge caused by active resolve/merge work that can be retried automatically SHALL advance into reducer-owned auto-resumable handling (`ResolveWait` or immediate resolving when promoted).

A deferred merge caused by a dirty base or other manual intervention requirement SHALL remain in manual merge wait handling (`MergeWait`).

The implementation MUST NOT infer auto-resumable versus manual-wait behavior by parsing a human-readable deferred reason string.

#### Scenario: auto-resumable-deferred-archive-promotes-to-resolve-wait

**Given**: A change is archived in parallel mode and merge is deferred because another resolve is already active
**When**: The deferred merge result is processed
**Then**: The change enters auto-resumable deferred handling (`ResolveWait` or equivalent queued resolve intent)
**And**: this decision does not depend on parsing a free-form reason string

#### Scenario: dirty-base-deferred-archive-stays-merge-wait

**Given**: A change is archived in parallel mode and merge is deferred because the base branch is dirty while no resolve is active
**When**: The deferred merge result is processed
**Then**: The change remains in manual merge wait handling (`MergeWait`)
**And**: it is not classified as auto-resumable
