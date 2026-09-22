# Design

## Authority and boundary

The proposal's structured `verifications` block is the only blocking command authority. At Acceptance start, Conflux selects declarations satisfying all of:

- `phase: pre-integration`
- `execution_class: repository-local`
- `completion_role: change-blocking`
- a valid runtime-supervisable command

Conflux materializes that set into a versioned execution manifest. The semantic reviewer cannot enlarge the set. It can inspect the candidate diff and repository files needed to judge implementation, but commands it starts outside the manifest are never awaited, reused, or counted as blocking evidence.

## Execution manifest

The manifest is stored atomically in the existing Git-excluded runtime evidence directory. It contains:

- schema version and change ID;
- full candidate commit and tree object IDs;
- full review base commit and exact `base..candidate` range;
- digest of canonical proposal, task artifact, spec deltas, constitution, and selected Acceptance skill;
- one normalized entry per eligible declaration: verification ID, argv, cwd, automation path/blob ID, resolved tool identity, artifact path/digest, and reuse outcome;
- invocation start, review deadline, effective absolute deadline, terminal outcome, cleanup proof, and stable terminal reason.

The input fingerprint is SHA-256 over canonical JSON containing every field that can change review authority or gate validity, excluding timestamps and terminal output. Array ordering is deterministic by verification ID and repository path.

## Gate execution

The runtime validates existing bound evidence first. Missing or stale evidence invokes the declared command by wiring the CLI-only `RuntimeVerificationExecutor` into Acceptance. Gates run before semantic review. A failed gate produces `fail`. A validated external prerequisite produces the existing resumable external hold. A runtime execution defect or deadline exhaustion produces a non-resumable `stalled` execution hold. Successful exact evidence is reused.

The semantic reviewer receives immutable gate outcomes. It does not receive authority to rerun or supplement blocking gates. This preserves semantic review: passing tests do not imply acceptable implementation, but unrelated tests cannot become an accidental completion condition.

## Deadline partition

Let `absolute_deadline` be the current effective Acceptance command deadline. Gates and semantic review share this budget. Reserve the final 30 seconds for finalization and cleanup; because the configured minimum is 300 seconds, at least 270 seconds remain for gates and review. Gate exhaustion uses category `acceptance_declared_gate_deadline_exhausted`.

At the review deadline, Conflux stops accepting reviewer output, terminates the owned process group through the existing cleanup path, and classifies the invocation from repository evidence and any complete canonical verdict already observed. Without a complete verdict, it emits non-resumable `stalled` with category `acceptance_review_deadline_exhausted`. If cleanup cannot be proven, the category is `acceptance_review_cleanup_unproven`. Deadline termination bypasses missing-verdict retry. The outer absolute runtime limit remains a defect-containment fallback, not the normal user-visible result.

## Retry semantics

A terminal deadline hold stores its manifest in the workspace evidence directory. Before shared admission, including automatic mark settlement, run-control recomputes the current fingerprint.

- Same fingerprint and prior non-resumable deadline category: refuse with `unchanged_acceptance_input`; dispatch nothing and list the fingerprint components that can restore eligibility.
- Different fingerprint: accept one retry edge. Revalidate every stored gate record and reuse only still-exact evidence.
- Prior `fail` with repository-fixable findings: retain existing repair semantics; the fingerprint guard does not replace finding identity and progress checks.
- External prerequisite holds retain their existing typed unblock contract.

Every retry alias calls the same classifier, so API snapshots and command admission cannot disagree.

## Constitutional fit

The manifest and evidence live in the worktree's existing ignored evidence directory. Recomputing from the same files and Git state yields the same fingerprint and retry decision after restart. Global state, logs, UI state, and external caches remain observability only.

## Rejected alternatives

- Prompt-only wording: cannot prevent a reviewer from starting and waiting on undeclared work.
- Per-change runtime configuration: violates the requested product contract and makes correctness depend on operator tuning.
- Increasing the timeout: extends the same unbounded exploration.
- Trusting proposal or agent-authored PASS prose: violates truthful completion and bound-evidence requirements.
- Generic shell sandboxing: broader than required and cannot reliably constrain every supported third-party agent runtime.
