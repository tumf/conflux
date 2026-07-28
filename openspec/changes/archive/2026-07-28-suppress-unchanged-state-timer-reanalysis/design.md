## Context

Conflux has two distinct re-analysis replay mechanisms:

1. a debounce-bypass edge reason can remain armed across timer wakes;
2. an ordinary timer reason can repeatedly pass a one-way queue-coalescing threshold after the queue has been stable for ten seconds.

The archived `prevent-repeated-resolve-completion-analysis` change fixed the first mechanism by consuming completion, repair, and slot-recovery reasons after one queued evaluation. Live v0.6.200 evidence confirms that fix: repeated evaluations now use `Initial`. The second mechanism remains because `last_queue_change_at` describes queue-coalescing age, not whether the current scheduler and repository inputs were already analyzed.

Dependency analysis may read proposal files and repository-visible dependency evidence. A queued-ID-only signature is therefore unsafe: the same IDs can mean different analysis input after proposal edits or base integration. Conversely, a time-only cooldown cannot establish quiescence; it only changes the period of an infinite loop.

## Decision

Introduce a process-local `AnalysisInputSignature` owned by the active scheduler runtime.

The signature represents the input that can change the dependency-analysis or dispatch decision. It must be deterministic and include:

- queued change analysis fields in stable ID order;
- prompt-relevant proposal input, including declared dependencies and frontmatter fields used by analysis;
- stable content digests of every queued and in-flight `proposal.md` path referenced by the prompt;
- sorted in-flight change IDs;
- available dispatch capacity and occupancy inputs that affect it;
- the effective dependency-base revision or an equivalent authoritative git/tree generation used by dependency classification.

The exact representation may use owned structured values or a stable hash produced with an already available repository mechanism. It must not use `DefaultHasher` or another deliberately unstable hash as a cross-comparison contract. The signature remains in memory and is compared only within the running process.

## Evaluation Boundary

Each scheduler iteration continues to perform cheap work first:

1. dynamic queue ingestion;
2. reducer-owned wait synchronization;
3. queued candidate reconciliation;
4. repository-visible queue classification and blocker checks;
5. debounce eligibility and effective reason calculation.

Immediately before expensive dependency analysis, the scheduler captures the current analysis-input signature.

For ordinary `Initial` evaluation only:

- if the captured signature equals the last completed signature, skip analyzer invocation and emit a deduplicated `unchanged_analysis_input` diagnostic;
- if it differs, invoke analysis normally.

For `QueueNotification`, `ResolveCompletion`, `RepairCandidate`, and `SlotRecovery`, do not suppress the edge evaluation based on signature equality. The edge remains one-shot and the newly captured signature replaces the last completed signature after a usable result.

This placement avoids caching queue classification itself. Repository-visible candidate eligibility remains freshly evaluated, while duplicate LLM work is removed.

## Completion and Fallback Semantics

The public analyzer closure currently returns only `AnalysisResult`, while LLM execution failure is converted to a metadata-dependency fallback before the scheduler receives it. The implementation must extend this internal runtime contract with provenance: `HealthyLlm`, `IntentionalMetadataOnly`, or `RecoverableFailureFallback`. This provenance is process-local execution metadata, not workflow authority.

Therefore:

- healthy LLM output and intentionally configured metadata-only output record a non-expiring captured signature;
- recoverable-failure metadata fallback records the captured signature as degraded with a fixed five-minute expiry;
- after expiry, exactly one ordinary eligible analysis retry is permitted for unchanged input and may replace the degraded record;
- an empty or otherwise unusable result that terminates before a dependency-analysis decision does not record a completed signature;
- a result that selects no dispatch while capacity is positive and `in_flight` is empty does not record a completed signature;
- recording happens against the pre-analysis snapshot, not a post-analysis recomputation.

Bounded degraded suppression prevents a broken LLM command from being relaunched on every timer wake while still recovering after a transient outage. A real queue, edge, capacity, proposal, or repository revision change re-arms analysis immediately.

## Signature Failure and Probe Cost

The normative fingerprint is a stable digest of the exact analyzer-input materials: queued `Change` analysis fields in stable ID order, queued and in-flight proposal file contents referenced by the prompt, sorted in-flight IDs, available capacity, and effective dependency-base revision.

Signature construction is fail-open. Any proposal-read or revision-resolution error returns `SignatureUnavailable`, emits a deduplicated warning, permits analysis, records no signature, and does not escape the scheduler loop.

The ordinary scheduler wakes every 500 ms, but signature probing must not run a VCS command at that frequency. Once an input is suppressed, the next fingerprint/revision probe is eligible no more than once per existing ten-second queue debounce interval. Wakes before that deadline use the process-local suppression deadline only to avoid work; they do not inspect VCS or proposal files. Explicit edge reasons bypass suppression and proceed directly to fresh evaluation. If an existing repository-event-maintained revision is available, it may re-arm sooner; otherwise `get_current_revision()` is called only at the bounded probe point.

## Repository Fingerprint

The effective dependency-base revision is obtained through the existing VCS workspace abstraction at the bounded probe point. Dirty or uncommitted proposal input is represented independently by stable content digests of all queued and in-flight proposal paths emitted in the analyzer prompt.

A different mechanism is allowed only if implementation evidence proves byte-equivalent invalidation for every queued/in-flight proposal input and effective-base change covered by the normative digest. A timestamp, queued ID set, or worktree status category alone is insufficient. Signature tests must mutate same-ID queued and in-flight proposal content independently and observe inequality.

## Invariants

- Same analysis input plus timer-only wake does not invoke the analyzer twice.
- Explicit scheduler edges remain immediate and one-shot.
- Cheap queue/repository classification is not cached or suppressed.
- Capacity recovery changes the signature even if its notification edge is lost, providing a bounded timer fallback.
- Proposal or effective-base evidence changes re-arm analysis without changing queued IDs.
- Signature state is runtime-only and non-authoritative under Constitution law 1.
- Process restart performs an initial analysis rather than trusting previous logs or cache state.
- No prior `AnalysisResult` is reused to authorize dispatch.

## Rejected Alternatives

### Analysis-completion cooldown

Rejected because it converts continuous analysis into periodic continuous analysis. It cannot prove the input changed and still repeats indefinitely.

### Reset `last_queue_change_at` after analysis

Rejected because it corrupts the meaning of queue-coalescing state and merely produces a ten-second retry loop.

### Skip ordinary analysis only at zero capacity

Rejected because it solves only the observed state, can depend on perfect slot-recovery signaling, and leaves unchanged-input repetition possible when positive capacity exists but no candidate dispatches.

### Queue and in-flight IDs only

Rejected because proposal content, dependency metadata, and effective-base integration can change while IDs remain equal.

### Reuse the previous `AnalysisResult`

Rejected because runtime analysis output must not become hidden authoritative workflow state. Fresh explicit edges and repository changes must invoke analysis.

### Long periodic safety retry for healthy results

Rejected because a healthy unchanged input should remain quiescent. The only periodic retry in this change is the fixed five-minute re-arm for a degraded recoverable-failure fallback, where recovery from a transient analyzer outage is a demonstrated requirement.

## Test Strategy

Use deterministic signature unit tests and scheduler-loop tests with paused Tokio time and an analyzer invocation counter.

Required cases:

- stale `last_queue_change_at`, unchanged queued/in-flight/capacity/revision state, and many 500 ms timer advances invoke analysis exactly once;
- the same test passes when the mocked analysis duration is below ten seconds and when it exceeds ten seconds;
- a queue addition bypasses the matching signature and invokes analysis once;
- each completion, repair, and slot-recovery edge bypasses once, then later timer wakes remain suppressed;
- capacity changes without relying on a `SlotRecovery` reason invalidate the signature and reach dispatch evaluation;
- changing dependencies or prompt-relevant proposal input for the same change ID invalidates the signature;
- changing only effective dependency-base revision invalidates the signature;
- a metadata fallback result records a degraded signature, suppresses rapid retries, and permits exactly one retry after five minutes; a subsequent healthy result becomes non-expiring;
- an unusable empty result does not incorrectly suppress a later eligible attempt;
- positive capacity with no in-flight work and zero selected dispatch does not arm suppression and remains eligible at the next debounced timer evaluation;
- queued and in-flight same-ID proposal edits independently invalidate the signature;
- signature-read and revision failures fail open without panic or recorded suppression;
- suppressed 500 ms wakes before the ten-second probe deadline perform no proposal reads or VCS revision command;
- a queue/proposal change during analysis is detected on the next eligible probe because the stored signature reflects the pre-analysis snapshot;
- a new executor/runtime starts without a previous signature.

All default tests must finish under one second. Timer behavior must use paused time rather than wall-clock sleeps.
