# Design: Separate recovery metadata from spine evidence

## Decision

Use separate typed Git observation methods for recovery metadata and evidence-bearing spine commits.

The recovery method returns only the fields its scanners consume: commit SHA, ordered parent SHAs, and raw message. The spine method continues returning `SpineCommit` with `CommitTreeEvidence`.

## Rationale

Making tree evidence optional on `SpineCommit` would spread partial-state handling into safety-critical classification and permit accidental validation with missing evidence. A boolean such as `include_tree_evidence` would hide two different correctness contracts behind one method and make call-site mistakes easy. Separate types make the inexpensive and evidence-bearing paths explicit.

The metadata walk uses the existing framed `git log --first-parent` format and parser behavior. It preserves oldest-first output and the optional limit while removing only per-record `tree_evidence` calls.

## Runtime Flow

```text
TUI / finite run startup
  ensure_no_unpushed_upstream_recovery
    scan_pending_publications
      first_parent_metadata(limit=500)
    scan_unpushed_upstream_merges
      first_parent_metadata(limit=500)

Enabled upstream integration
  first_parent_commits(from_fetched, local_head)
    git log metadata
    tree_evidence(commit) for each spine commit
  validate_spine
```

The two recovery scanners may continue issuing ancestry/ref commands only for commits with matching, structurally valid trailers. The ordinary no-match path performs two bounded log commands total and no tree commands.

## Safety Invariants

- Recovery remains workspace-local and derives decisions only from Git state.
- Recovery remains complete before orchestration dispatch or mutation.
- The existing 500-commit bound and first-parent semantics remain unchanged.
- An upstream merge trailer is recovery evidence only when its recorded SHA is a non-first parent of the merge commit.
- A pending publication marker remains bound to its recorded remote and branch.
- Full spine validation never substitutes empty/default tree evidence for an unavailable tree read.

## Verification Strategy

- Unit tests use a fake Git port that can fail if the wrong observation method is called.
- Native Git fixture tests validate framing, ordering, limits, and absence of tree commands on recovery metadata reads.
- Existing coordinator and spine tests retain semantic coverage for refusal and archive evidence.
- A heavy ignored benchmark asserts subprocess-count shape rather than a machine-specific elapsed-time threshold; elapsed time is retained as diagnostic evidence.

## Alternatives Rejected

- Run recovery after first draw: rejected because unpublished integration must block before orchestration mutation and finite run has no TUI draw boundary.
- Cache recovery results: rejected because cache state cannot be authoritative under the workspace-local constitution and does not remove cold-start cost.
- Parallelize 2,000 `ls-tree` calls: rejected because the data is unused and subprocess amplification remains.
- Remove recovery from option-less startup: rejected because it violates canonical recovery requirements.
