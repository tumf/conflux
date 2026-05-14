# Design: ResolveWait clear outcome naming

## Current Behavior

`ParallelExecutor::clear_resolve_wait_intent_for_success` removes a change from executor-local retry sets and clears the same `ResolveWait` membership from the shared reducer. Although originally named around successful merge outcomes, the helper is also used when retry work is no longer actionable because the archived workspace is missing, the workspace path is stale, or the change is already merged to base.

## Decision

Use neutral outcome-oriented naming for the executor helper where practical. The preferred name is:

```rust
clear_resolve_wait_intent_for_outcome
```

This name communicates that the helper clears retry intent after a scheduler-observed outcome, without implying the outcome was a successful merge.

If a rename is unexpectedly too broad or risky, add comments instead. The comments must state that the helper is intentionally used for successful merge, already-merged detection, and stale/missing retry-prerequisite cleanup because all of those outcomes mean reducer-owned `ResolveWait` should not remain pending.

## Behavior Preservation

This proposal is a maintainability-only implementation change. It must not change:

- retry dispatch ordering;
- reducer-owned `ResolveWait` / `MergeWait` transitions;
- dirty-base demotion behavior;
- stale/missing workspace cleanup outcomes;
- success or failure event emission.

## Verification Strategy

The focused tests from the recent zero-change startup fix are the primary guardrail because they prove stale/missing workspace handling clears both executor-local and reducer-owned retry membership while preventing indefinite `resolve pending` display.
