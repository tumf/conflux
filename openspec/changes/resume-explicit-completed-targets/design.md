# Design: repository-evidence explicit target resume

## Context

Explicit target normalization produces `Some(Vec<String>)`, then `filter_requested_changes()` joins those IDs only against `list_changes_native()`. Archive removes a completed change from that list. Existing parallel resume discovery occurs later, so it cannot rescue the invocation.

The target resolver must run before mutation, distinguish completed from resumable, and reject fabricated or contradictory state. It cannot use server DB, lifecycle events, logs, or commit-message convention as authority.

## Decisions

### Decision: classify requested IDs, do not rewrite them

The resolver retains every normalized requested ID and its order. It produces classifications rather than a shorter anonymous `Vec<Change>`. This allows scheduler input, dry-run output, and supervised terminal output to agree without server-side target recomputation.

Duplicates remain invalid because allowing them would make result arrays and scheduling ambiguous.

### Decision: active evidence takes metadata, base evidence proves completion

An active OpenSpec entry supplies the `Change` metadata required by scheduling. Base-integrated completion uses the existing constitutional tree contract:

- an exact or date-prefixed `openspec/changes/archive/<id>` entry exists in the captured base branch tree;
- `openspec/changes/<id>` is absent from that same tree.

Archive plus active directory is contradictory, not completed. Commit subject and branch name are irrelevant.

### Decision: worktree resume requires content evidence

A worktree discovered through the configured workspace manager is only a candidate. The resolver inspects its existing state through the same phase detector used for resume. The worktree must contain active-change or archive evidence for the requested ID and have readable Git state. A matching workspace/branch name alone is rejected.

This catches applied, archiving, archived-not-integrated, and other supported phase states without introducing a second workflow state machine.

### Decision: resolve all errors before mutation

The resolver gathers duplicates, unknown IDs, contradictory base state, and invalid worktree evidence across the full request, then returns one deterministic diagnostic. It does not create, clean, delete, or replace any worktree while classifying.

### Decision: `--no-resume` cannot destroy evidence

`--no-resume` changes only worktree reuse eligibility. Base-integrated targets remain completed because deleting worktrees cannot undo base evidence. A target that exists only as valid worktree resume evidence causes a pre-mutation error explaining that `--no-resume` cannot continue that target safely; it does not delete the worktree and recreate from an absent active proposal.

### Decision: one resolver for parallel dry-run, ordinary run, and upstream run

The repository classification is a parallel-run concern, not an upstream-specific feature. `-u` consumes it so a supervisor can resubmit the same set, but ordinary cumulative parallel runs receive the same idempotency. Dry-run invokes the read-only resolver and displays the classifications.

Serial behavior remains unchanged because serial is obsolete and has different loop semantics.

## Data Flow

```text
normalized requested IDs
  -> capture base branch identity
  -> load active OpenSpec changes
  -> inspect base tree completion evidence
  -> discover candidate managed worktrees
  -> validate candidate workspace phase evidence
  -> aggregate duplicate/unknown/evidence errors
  -> TargetResolution
       active/resumable -> scheduler initialization
       already_completed -> successful skip evidence
       errors -> pre-mutation failure
```

## Failure Handling

- Missing/detached base identity fails before classification can claim completion.
- Git command failure is an evidence error, not `unknown` and not completed.
- Missing candidate path, unreadable worktree Git state, or contradictory content is an evidence error.
- Unknown means the inspections succeeded and found no active, completed, or valid resume evidence.
- No failure path deletes or normalizes repository evidence.
