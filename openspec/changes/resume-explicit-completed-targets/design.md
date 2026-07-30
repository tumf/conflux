# Design: repository-evidence explicit target resume

## Context

Explicit target normalization produces `Some(Vec<String>)`, then `filter_requested_changes()` joins those IDs only against `list_changes_native()`. Archive removes a completed change from that list. Existing parallel resume discovery occurs later, so it cannot rescue the invocation.

The target resolver must run before mutation, distinguish completed from resumable, and reject fabricated or contradictory state. It cannot use server DB, lifecycle events, logs, or commit-message convention as authority.

## Decisions

### Decision: classify requested IDs, do not rewrite them

The resolver retains every normalized requested ID and its order. It produces classifications rather than a shorter anonymous `Vec<Change>`. This allows scheduler input, dry-run output, and supervised terminal output to agree without server-side target recomputation.

Duplicates remain invalid because allowing them would make result arrays and scheduling ambiguous.

### Decision: active evidence takes metadata, base evidence proves completion

An active OpenSpec entry supplies the `Change` metadata required by scheduling and takes classification precedence when a managed worktree also exists; existing scheduler resume discovery may subsequently reuse that worktree. Base-integrated completion uses the existing constitutional tree contract:

- an exact or date-prefixed `openspec/changes/archive/<id>` entry exists in the captured base branch tree;
- `openspec/changes/<id>` is absent from that same tree.

The current Boolean helper is insufficient for diagnostics and fail-safe routing. The resolver factors its tree checks into `Completed`, `NotCompleted`, `Contradictory`, and `EvidenceError`; archive plus active directory is contradictory, Git/branch read failure is an evidence error, and neither may collapse into not-completed/unknown. Commit subject and branch name are irrelevant.

### Decision: worktree resume requires content evidence

A worktree discovered through the configured workspace manager is only a candidate. The resolver inspects its existing state through the same phase detector used for resume. The worktree must contain active-change or archive evidence for the requested ID and have readable Git state. A matching workspace/branch name alone is rejected.

This catches applied, archiving, archived-not-integrated, and other supported phase states without introducing a second workflow state machine.

### Decision: resolve all errors before mutation

The resolver gathers duplicates, unknown IDs, contradictory base state, and invalid worktree evidence across the full request, then returns one deterministic diagnostic. It does not create, clean, delete, or replace any worktree while classifying.

### Decision: `--no-resume` cannot destroy evidence

`--no-resume` changes only worktree reuse eligibility. Base-integrated targets remain completed because deleting worktrees cannot undo base evidence. A target that exists only as valid worktree resume evidence causes a pre-mutation error explaining that `--no-resume` cannot continue that target safely; it does not delete the worktree and recreate from an absent active proposal.

### Decision: one resolver with explicit upstream ordering

The repository classification is a parallel-run concern, not an upstream-specific feature. Ordinary runs capture the attached local base identity and classify before first dispatch. Enabled real `-u` runs capture identity, complete the mandatory initial upstream base-lane checkpoint, and then classify from the resulting current cumulative base before any change-worktree creation or reuse registration. This ordering sees completion newly integrated from upstream and avoids dispatching stale active targets. Dry-run performs no network fetch, so it classifies read-only against the current local base and states that limitation in output.

An all-already-completed `-u` classification does not short-circuit upstream finalization. If the initial checkpoint/startup history classification finds recognized unpublished cumulative or upstream recovery history, the existing upstream zero-change recovery path still performs verification, native push, and remote confirmation. Fresh no-work with no such history remains the upstream proposal's no-push completion case.

Serial behavior remains unchanged because serial is obsolete and has different loop semantics.

## Data Flow

```text
normalized requested IDs
  -> capture attached base identity
  -> when real -u: complete initial upstream checkpoint
  -> select resulting current cumulative base (dry-run: current local base)
  -> load active OpenSpec changes
  -> inspect typed base tree completion evidence
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
- An uncommitted archive in the base checkout but absent from the selected base commit tree is not completion evidence; without valid managed-worktree evidence it remains unknown rather than being promoted from working-copy appearance.
- Missing candidate path, unreadable worktree Git state, or contradictory content is an evidence error.
- Unknown means the inspections succeeded and found no active, completed, or valid resume evidence.
- No failure path deletes or normalizes repository evidence.
