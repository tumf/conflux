## MODIFIED Requirements

### Requirement: Resume options and frontends have explicit boundaries

The repository-evidence resolver MUST apply to ordinary and upstream-enabled cumulative explicit-target runs. A real upstream-enabled run MUST complete its initial upstream checkpoint before classification; dry-run MUST perform no network fetch and MUST classify read-only against the current local base. `--no-resume` MUST preserve already-completed classification but MUST reject targets whose only valid evidence is a resumable worktree without deleting it. `--all` behavior MUST remain unchanged. An all-already-completed upstream-enabled result MUST NOT bypass the existing zero-change recovery/finalization path when recognized unpublished cumulative or upstream history exists.

#### Scenario: no-resume retains base completion

**Given**: one target is already completed in base and another exists only as a resumable worktree
**When**: the explicit run uses `--no-resume`
**Then**: the completed target remains a successful skip
**And**: Conflux rejects the worktree-only target before deleting its evidence

#### Scenario: dry-run reports without mutation

**Given**: explicit targets include active, completed, resumable, and unknown IDs
**When**: dry-run resolves them
**Then**: it reports each classification and unknown error deterministically
**And**: it performs no worktree creation, reuse registration, cleanup, merge, or archive mutation
