## ADDED Requirements

### Requirement: Proposal guidance MUST avoid duplicate hook-owned verification tasks

The bundled `cflx-proposal` skill MUST inspect tracked repository hook configuration before delegating repository-wide format, lint, test, or generated-artifact verification to final commit hooks. It MUST omit such checks as independent checkbox tasks only when the tracked hook runs them unconditionally, including on the clean-tree amend path. Staged-file-only hooks MUST NOT qualify. Requirement-specific tests MUST remain attached to implementation tasks, and heavy or E2E checks MUST retain an explicit verification owner outside pre-commit unless the repository explicitly defines otherwise.

#### Scenario: Unconditional tracked hook owns repository-wide check

**Given**: tracked repository hook configuration runs a repository-wide lint or fast-test command unconditionally
**When**: `cflx-proposal` creates implementation tasks
**Then**: it does not add a separate checkbox whose only work is rerunning that hook-owned command
**And**: requirement-specific test implementation and verification remain represented

#### Scenario: Staged-file hook cannot own amend verification

**Given**: a repository hook only receives or filters staged filenames
**And**: final Apply normally amends a clean WIP commit
**When**: `cflx-proposal` assigns verification ownership
**Then**: it does not treat that hook as authoritative repository-wide verification
**And**: the proposal keeps an explicit executable verification path

#### Scenario: Heavy verification remains separately owned

**Given**: a required integration or E2E check is excluded from default pre-commit execution
**When**: `cflx-proposal` creates the verification plan
**Then**: the check remains explicitly declared under an appropriate verification owner
**And**: the proposal does not silently move it into pre-commit or omit it

### Requirement: Apply guidance MUST separate staging from commit ownership

The bundled `cflx-apply` skill MUST require Apply agents to stage only change-owned files, leave no unstaged or untracked entries before declaring completion, and refrain from creating the final commit. It MUST require foreground completion of verification work and MUST prohibit returning while a background verification command remains active. Conflux remains responsible for WIP preservation, repository-hook execution, and final commit creation.

#### Scenario: Apply agent prepares but does not commit

**Given**: an Apply agent has completed repository changes
**When**: it prepares the workspace for finalization
**Then**: it stages the intended change-owned files
**And**: it verifies there are no unstaged or untracked entries
**And**: it does not create the final commit

#### Scenario: Apply agent waits for verification completion

**Given**: an Apply agent starts a verification command required by its task
**When**: the command is still running
**Then**: the agent does not return a final response with that command left in the background
**And**: it waits for completion or records a valid blocker under existing Apply rules
