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
- a repository-visible fingerprint for proposal content the configured analyzer can read;
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

The public analyzer closure currently returns `AnalysisResult`, and LLM execution failure is converted to a metadata-dependency fallback before the scheduler receives it. That fallback is a usable degraded analysis result, not a terminal analyzer failure.

Therefore:

- successful LLM output records the captured signature;
- successful metadata fallback records the captured signature;
- an empty or otherwise unusable result that terminates before a dependency-analysis decision does not record a completed signature;
- recording happens against the pre-analysis snapshot, not a post-analysis recomputation.

Treating fallback as completed prevents a broken or unavailable LLM command from being relaunched on every timer wake. A real queue, edge, capacity, proposal, or repository revision change still re-arms a retry.

## Repository Fingerprint

The preferred repository component is the effective dependency-base revision already exposed by the VCS workspace abstraction. Proposal input that may be dirty or not represented by base `HEAD` must also be represented. The implementation must choose the smallest repository-visible fingerprint that changes when files read by the analyzer change.

Acceptable implementations include:

- a stable digest of the proposal files and prompt metadata for queued/in-flight changes plus effective base revision;
- a repository tree/worktree status fingerprint scoped to those proposal paths plus effective base revision;
- an existing equivalent repository generation if code inspection proves it covers both committed integration and relevant working-tree proposal changes.

A timestamp string alone is insufficient unless its producer is proven to change for every relevant content change. The signature unit tests must mutate same-ID proposal input and observe inequality.

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

### Long periodic safety retry

Not included in the initial design. A periodic retry weakens the requirement that timer-only unchanged input remain quiescent. It is unnecessary if the signature covers all actual analysis inputs and effective repository evidence. Add it only through a later change backed by a demonstrated unobservable input.

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
- a metadata fallback result records the signature and prevents repeated failing-LLM invocation for unchanged timer state;
- an unusable empty result does not incorrectly suppress a later eligible attempt;
- a queue/proposal change during analysis is detected on the next loop because the stored signature reflects the pre-analysis snapshot;
- a new executor/runtime starts without a previous signature.

All default tests must finish under one second. Timer behavior must use paused time rather than wall-clock sleeps.
