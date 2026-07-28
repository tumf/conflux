## Context

The archived unchanged-input suppression change correctly separated one-shot scheduler edges from ordinary timer evaluation and introduced runtime-only healthy/degraded signatures. Review of the v0.6.201 implementation found that the signature and loop do not yet preserve all of the liveness assumptions declared by that design.

Three failure modes interact at the same boundary:

1. signature invalidation observes checkout `HEAD`, while dependency classification observes a selected effective-base ref;
2. an unusable empty analysis ends the scheduler while queued work remains;
3. fail-open signature construction has no bounded failure retry state and can therefore become a 500 ms fail-hot loop.

A correction must preserve the original quiescence guarantee while making every non-quiescent state eventually eligible again.

## Decision

### Share effective-base identity and revision

Dependency classification and analysis signature generation must consume one authoritative effective-base identity. The identity includes the selected branch/ref and its resolved revision. Signature probing must not substitute the currently checked-out commit unless that commit is proven to be the selected effective base.

The effective-base resolver may be factored into a shared helper or exposed through the existing dependency state, but it must not duplicate branch-selection logic in two independently drifting implementations. Revision resolution errors follow the bounded fail-open path.

### Separate completed suppression from failed-probe throttling

A completed signature proves that a usable result was obtained for a known input. A failed probe proves no such thing. They must remain distinct runtime states.

- `CompletedAnalysisInput` continues to suppress a matching healthy input indefinitely or a degraded input until expiry.
- A signature-unavailable timer attempt records only a process-local next ordinary retry deadline.
- Before that deadline, ordinary timer wakes skip both signature probing and analyzer invocation and emit a deduplicated bounded-retry reason.
- At the deadline, the scheduler retries signature construction and analysis.
- Explicit queue, completion, repair, and slot-recovery edges bypass this timer deadline once.
- A successful probe and usable analysis clear failure throttling and establish the normal completed-input record.

This is rate-limited fail-open, not fail-closed caching. No failed input is treated as analyzed, and no durable state is written.

### Keep unusable results non-terminal while work exists

An empty analysis order is not evidence that queued work completed. When reducer-visible queued work remains, the attempt must return a non-terminal no-progress decision, leave the captured signature unrecorded, and let the scheduler wait until the next debounce-eligible timer evaluation or explicit event.

The scheduler may terminate only through its canonical fully-drained or terminal-outcome checks. An analyzer output shape must not create an alternate drain definition.

### Canonicalize semantic analyzer input

The signature intentionally models queued/in-flight membership as deterministic semantic input rather than random collection iteration. The analyzer and metadata fallback should receive the same canonical ordering. This avoids both false signature mismatches and order-sensitive prompt/fallback behavior without making `HashSet` iteration order authoritative.

### Compose deadlines by earliest semantic expiry

For a healthy completed signature, set the next repository probe when the completed record is installed, so the immediate 500 ms wake does not perform I/O.

For a degraded completed signature, the next probe deadline is the earlier of:

- the ten-second repository probe cadence; and
- the degraded record's five-minute expiry.

This preserves both the VCS frequency bound and the exact bounded recovery promise.

## Invariants

- A named effective-base ref change invalidates the signature even when checkout `HEAD` does not change.
- Queued work plus an unusable analyzer result never becomes a false fully-drained state.
- Persistent signature failure never launches a timer-driven analyzer or VCS probe every 500 ms.
- Fail-open retry remains finite and autonomous; recovery needs no user action or queue membership change.
- Explicit state-transition edges retain one immediate evaluation.
- Healthy unchanged input stays quiescent without periodic LLM retries.
- Degraded unchanged input retries at its five-minute expiry, not after an additional probe interval.
- Process-local deadlines and signatures do not become durable workflow authority.

## Rejected Alternatives

### Use checkout HEAD as a practical approximation

Rejected because the canonical dependency requirement explicitly permits original and stacked integration bases. An approximation can suppress the only timer evaluation that observes a dependency ref advancing.

### Treat empty order as successful completion

Rejected because queued reducer intent remains. Ending the loop hides work rather than resolving it.

### Probe fail-open state on every wake

Rejected because it recreates the original expensive replay loop under a persistent VCS or filesystem failure.

### Record a fake completed signature after probe failure

Rejected because unavailable evidence is not completed analysis. This would turn fail-open into silent starvation and violate truthful completion.

### Make signature sensitive to input iteration order

Rejected because collection iteration is not repository-visible semantic change and can recreate repeated analysis. Canonicalize the consumer input instead.

## Test Strategy

Use paused Tokio time and the real orchestration loop wherever termination, wake cadence, or retry timing is under test.

Required cases:

- the effective-base branch ref advances while checkout `HEAD`, queued IDs, proposal content, in-flight membership, and capacity remain unchanged; the next bounded probe re-arms analysis;
- an empty order with queued work and no in-flight work does not exit the full loop and retries at the next debounce-eligible timer evaluation;
- a truly drained scheduler retains its configured existing termination or persistent-wait behavior;
- persistent revision failure and persistent proposal-read failure each produce at most one timer-driven probe/analyzer attempt per ten seconds across repeated 500 ms wakes;
- an explicit edge during that failure interval gets one immediate attempt, then timer throttling resumes;
- a failed probe later succeeds and transitions into ordinary completed-signature suppression;
- a healthy completed result performs no immediate 500 ms proposal/VCS re-probe;
- a matching degraded result probed at 4m59s retries on the first eligible wake at or after 5m00s;
- queued and in-flight order-only permutations remain one semantic signature and one deterministic analyzer input;
- all previous unchanged-input, edge-bypass, positive-capacity zero-dispatch, capacity recovery, proposal digest, and process-reset regressions continue to pass.

Every new default-path test must complete under one second using paused time.
