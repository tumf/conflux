## MODIFIED Requirements

### Requirement: Acceptance blocker input compatibility is distinct from lifecycle display taxonomy

The canonical parser MAY continue to accept `gated` and legacy `blocked` verdict input for compatibility, but runtime MUST distinguish bare compatibility input from a validated structured external blocker. Bare input without explicit supported category, concrete non-empty evidence, next action, and resumability MUST be treated as an Acceptance protocol error and MUST NOT create `stalled`, dependency `blocked`, or an inferred blocker category.

A validated external blocker SHALL enter the non-terminal user-facing `stalled` lifecycle. Newly authored lifecycle/status surfaces MUST NOT expose `gated` as a status. Dependency wait remains the only `blocked` display meaning.

#### Scenario: bare gated input receives bounded protocol retry

- **GIVEN** Acceptance emits `{"acceptance":"gated"}` or `ACCEPTANCE: GATED` without a structured blocker payload
- **WHEN** runtime parses and routes the result
- **THEN** it classifies the result as an Acceptance protocol error
- **AND** it retries Acceptance only within the shared fixed protocol budget
- **AND** it emits no stalled lifecycle transition or blocker category
- **AND** it creates no change artifact or durable stalled record

#### Scenario: legacy bare blocked input is compatibility-only

- **GIVEN** an older integration emits a bare `blocked` Acceptance verdict
- **WHEN** a compatibility-aware runtime parses it
- **THEN** it follows the same bounded protocol-error path as bare `gated`
- **AND** it is not displayed as dependency `blocked` or execution `stalled`

#### Scenario: validated blocker displays as stalled

- **GIVEN** Acceptance emits a blocker with an explicit supported category, concrete evidence, next action, and resumability
- **AND** runtime verifies that repository-only Apply work cannot resolve the prerequisite
- **WHEN** runtime exposes lifecycle state
- **THEN** the displayed status is `stalled`
- **AND** the explicit category is preserved without prose-based inference
- **AND** new prompts and tests do not require `gated` as a lifecycle/display term

### Requirement: Acceptance stalled retry evidence is workspace-local

Ordinary Acceptance retry bookkeeping during an active serial or parallel run MUST remain in memory and MUST NOT use `.cflx/acceptance-state.json` or a worktree checkpoint. Acceptance MUST NOT create an Acceptance-origin `APPLY_BLOCKED/marker.md` or another change-directory artifact.

A validated Acceptance stalled hold MUST be stored in versioned, atomic Conflux runtime state outside the worktree. The record MUST bind repository identity, change ID, managed worktree identity/path, branch when available, Apply revision, stalled phase, retry count, explicit blocker category and evidence, resumability, next action, and timestamps. Runtime MUST reconcile that binding with current repository/Git/worktree facts before the record controls dispatch or retry.

Runtime state MAY control ordinary dispatch suppression, stalled presentation, explicit retry eligibility, and Acceptance resume phase. It MUST NOT prove implementation completion, Acceptance PASS, archive readiness, merge eligibility, or base integration. If state is absent or invalid while repository evidence shows a complete unarchived Apply revision, Conflux MUST run Acceptance again and MUST NOT infer PASS.

#### Scenario: validated stall survives restart without dirtying worktree

- **GIVEN** Acceptance records a validated resumable external blocker for a complete Apply revision
- **AND** the managed worktree is clean
- **WHEN** Conflux restarts and reconciles a matching runtime record
- **THEN** it restores execution `stalled` and the recorded next action
- **AND** ordinary dispatch starts neither Apply, Acceptance, nor archive
- **AND** the worktree remains clean and the Apply commit remains unchanged

#### Scenario: missing runtime state reruns Acceptance

- **GIVEN** a complete unarchived Apply revision exists
- **AND** no valid Acceptance stall record exists after restart
- **WHEN** Conflux derives the next action
- **THEN** it runs Acceptance again
- **AND** it does not infer prior PASS, enter archive, or rerun Apply solely from missing runtime metadata

#### Scenario: stale state cannot override repository evidence

- **GIVEN** a stored stall has a mismatched repository, worktree identity, path reuse guard, Apply revision, ancestry, or active-change state
- **WHEN** restart or retry reconciliation evaluates it
- **THEN** the record is invalidated or quarantined with a diagnostic
- **AND** routing is recomputed from repository/Git/worktree evidence
- **AND** the stale record cannot suppress cleanup or authorize archive or merge

#### Scenario: explicit retry resumes Acceptance transactionally

- **GIVEN** a valid resumable Acceptance stall matches the current Apply revision
- **WHEN** an operator explicitly retries it
- **THEN** runtime prepares and starts Acceptance without rerunning Apply
- **AND** the prior hold is consumed only across a successful dispatch-preparation boundary
- **AND** preparation failure retains the blocker evidence and does not dispatch ambiguous work

#### Scenario: legacy Acceptance marker migrates conservatively

- **GIVEN** a legacy marker is proven Acceptance-origin, resumable, structurally valid, and bindable to the current repository, worktree, and Apply revision
- **WHEN** Conflux performs one-time migration
- **THEN** it writes the runtime record before removing generated marker residue
- **AND** successful migration is idempotent and leaves the worktree clean
- **AND** Apply-origin, unknown-origin, non-resumable, malformed, or ambiguous markers are not silently migrated or deleted

### Requirement: Acceptance retry safeguards are mode-independent

Serial and parallel execution MUST use equivalent blocker validation, protocol retry, finding normalization, semantic progress, retry, mixed-blocker, stalled persistence, reconciliation, migration, and explicit-retry decisions.

Bare `gated` or legacy `blocked` input MUST share the fixed two-retry protocol bound used for missing verdict while retaining a distinct consecutive counter and corrective context. It MUST NOT consume Apply or explicit-CONTINUE budget, rerun Apply, or persist stalled state. Exhaustion MUST produce a terminal Acceptance protocol error requiring explicit retry.

The existing apply+Acceptance ceiling of ten cycles remains a safety ceiling. A validated repository-external blocker or cycle-exhaustion hold MAY become resumable runtime `stalled` only with explicit evidence; evidence-free exhaustion MUST NOT create a synthetic category or worktree marker.

#### Scenario: bare GATED budget is equivalent across modes

- **GIVEN** serial and parallel Acceptance each emit the same sequence of bare GATED results
- **WHEN** each applies protocol retry policy
- **THEN** both run at most two Acceptance-only retries after the initial result
- **AND** both return the same terminal protocol error on the third consecutive result
- **AND** neither writes stalled state or a worktree marker

#### Scenario: canonical verdict resets bare GATED sequence

- **GIVEN** a bare GATED result was retried
- **WHEN** the next Acceptance invocation returns a canonical PASS, FAIL, CONTINUE, or validated stalled blocker
- **THEN** the consecutive bare-GATED retry counter resets
- **AND** the canonical result follows its normal routing

#### Scenario: equivalent validated blockers produce equivalent state

- **GIVEN** serial and parallel observe equivalent validated structured external blockers for equivalent Apply revisions
- **WHEN** each computes and persists its decision
- **THEN** both preserve the same explicit category, evidence, resumability, next action, and revision binding
- **AND** both enter user-facing `stalled` without dirtying the worktree

### Requirement: Acceptance findings retain repository and external scopes

Runtime MUST classify findings individually as repository-fixable or external/non-mockable. Repository-fixable findings MUST remain actionable Apply repair input. External blockers MUST be retained when repository findings are present, but they MAY enter durable runtime `stalled` only after repository-fixable findings are resolved and the external blocker satisfies the structured validation contract.

Runtime MUST preserve an explicitly supplied supported category and MUST NOT infer credential, infrastructure, or other categories from narrative text. Missing or invalid blocker structure follows bounded protocol error rather than stalled persistence.

#### Scenario: mixed findings preserve both responsibilities

- **GIVEN** Acceptance identifies a repository defect and a concrete external prerequisite
- **WHEN** runtime evaluates the findings
- **THEN** the repository defect remains Apply-repairable
- **AND** the external prerequisite remains non-checkbox blocker metadata
- **AND** runtime does not stall before repository-fixable findings are resolved

#### Scenario: validated external blocker remains after repository repair

- **GIVEN** Apply resolves all repository-fixable findings
- **AND** Acceptance returns a valid structured external blocker
- **WHEN** runtime evaluates the result
- **THEN** it preserves the explicit blocker in revision-bound runtime stalled state
- **AND** it does not create a change-directory marker

#### Scenario: unsupported credential inference is prohibited

- **GIVEN** a bare or incomplete blocker narrative contains words such as credential, token, or auth
- **WHEN** runtime validates the result
- **THEN** it does not assign category `credential` from those words
- **AND** it follows bounded protocol-error handling until a valid structured category and evidence are supplied

### Requirement: Acceptance execution creates no JSON checkpoint

Serial and parallel Acceptance execution MUST NOT create, read, update, or delete `.cflx/acceptance-state.json`. Acceptance PASS for an active run MAY be held in memory only until archive handoff. After restart, incomplete archive work MUST be accepted again unless repository evidence already proves archive or base integration.

A versioned out-of-worktree Acceptance stall record is not a PASS checkpoint. It MAY represent only a validated temporary external hold bound to current repository/worktree/Apply evidence and MUST be ignored or invalidated when reconciliation fails.

#### Scenario: uninterrupted pass reaches archive without checkpoint

- **GIVEN** Apply completed and Acceptance runs in the same orchestration process
- **WHEN** Acceptance returns PASS
- **THEN** archive handoff proceeds for that accepted revision
- **AND** neither `.cflx/acceptance-state.json` nor a persisted PASS record exists

#### Scenario: runtime stall cannot substitute for PASS

- **GIVEN** a valid or stale Acceptance stall record exists
- **WHEN** Conflux evaluates archive readiness
- **THEN** the record cannot prove PASS or authorize archive
- **AND** Acceptance must pass for the current revision through the normal execution path

#### Scenario: runtime metadata cannot dirty post-archive worktree

- **GIVEN** Acceptance passes and archive artifacts are committed
- **WHEN** post-archive merge verification runs
- **THEN** no Acceptance runtime-state cleanup mutates the managed worktree
- **AND** no manual `MergeWait` is produced solely by runtime stall metadata

#### Scenario: genuine dirty evidence remains a blocker

- **GIVEN** archive artifacts are valid
- **AND** an unrelated user file remains modified
- **WHEN** post-archive merge verification runs
- **THEN** the unrelated dirty worktree remains concrete manual blocker evidence
- **AND** externalizing Acceptance stall state does not suppress the deferral
