# Design

## Limit selection

`CommandQueueConfig` carries both common and Acceptance limits. At the existing runtime-limit selection point, the runner inspects `operation_type`:

- `acceptance`: use the dedicated limit when common is zero; otherwise use `min(common, dedicated)`.
- every other operation type, including cleanup-review: use the common limit unchanged.

The runner method signature remains unchanged.

## Range

The dedicated key defaults to 1,800 seconds and accepts `300..=10,800`. The lower bound avoids presenting a skill-loading-scale interval as a generally safe production minimum. The upper bound preserves the existing common default as an explicit ceiling. A shorter positive common limit is an independent safety override and may produce an effective Acceptance limit below 300 seconds.

The 1,800-second default is a containment value, not a performance target: it is one sixth of the prior three-hour default and remains long enough for bounded repository review. Future tuning requires observed duration evidence.

## Terminal routing

After the runner proves owned-process quiescence, Acceptance consumes `CommandTermination::RuntimeLimit` as a dedicated terminal run outcome. Dispatch bypasses ordinary Acceptance failure routing, command-recovery retry, missing-verdict retry, and Apply re-entry. The command retry counter remains unchanged.

## Restart semantics

The guard bounds one run, not durable change history. Repository state remains authoritative, so owner restart may rerun Acceptance with a fresh budget. No external timeout marker controls the next action.

## Test clock

Tests inject sub-second limits or pause time using the existing command-runner precedent. Production validation still enforces the configured minimum.
