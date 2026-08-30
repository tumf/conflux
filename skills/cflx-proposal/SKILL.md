---
name: cflx-proposal
description: Create structured Conflux change proposals through interactive conversation with users. Use when users request "create a proposal", "draft a change", "propose a feature", or similar proposal creation tasks. This skill asks clarifying questions and guides users through the proposal process.
---

# Conflux Proposal Creator

Create structured change proposals for Conflux (OpenSpec-based) projects through interactive conversation with users.

## Scope Restrictions (Proposal-Only)

- This skill is for proposal creation only. Do NOT implement or modify product/source code.
- You may READ any files for context gathering.
- You may WRITE only under `openspec/changes/<change-id>/`.
- After strict validation passes, stop and present the proposal for review.

## From Problem to Change Contract

A reported problem is investigation input, not a proposal. Write proposal artifacts only after read-only repository evidence has answered what the current behavior is and why it happens, and only when that answer requires a permanent change.

1. **Establish current behavior and root cause first.** Read code, tests, specs, logs, and history within the proposal-only scope above: investigation is read-only, and it never authorizes editing product or test files. While the evidence still leaves the failing path, the root cause, the approach, or the acceptance criteria uncertain, keep investigating instead of formalizing the hypothesis as a proposal.
2. **Decide whether a permanent change is required at all.** When the investigation shows the behavior is already correct, that a transient environment fault caused it, or that a local repair resolves it without leaving any lasting contract, create no proposal and report the finding instead.
3. **Separate temporary work from the permanent change.** Diagnostic instrumentation, throwaway reproduction scripts, one-off data repairs, and local environment fixes are investigation artifacts. They belong in the report, never in the proposal's scope, tasks, or spec deltas. The proposal describes only the permanent transition the repository must make.
4. **Choose the implementation approach before writing tasks, and record scope-relevant rejected alternatives.** When a rejected alternative would have changed the proposal's scope or its preserved contracts, state that alternative and its consequence. Incidental exploration detail does not belong in the proposal.
5. **Write the change as a contract.** The proposal is implementation-ready only once it states:
   - the observable final state — what is true after the change that is not true now;
   - the change boundary — which files, modules, or surfaces this change may touch, and which it must leave alone;
   - the preserved contracts — existing behavior, APIs, formats, and invariants that must keep working unchanged;
   - the failure behavior — what the changed code does when its inputs, dependencies, or preconditions are wrong;
   - the repository-local acceptance — the bounded command or check that proves the final state from repository evidence.
6. **Leave no design decision to the implementation agent.** Conflux decides execution order autonomously, so every task must be executable from the proposal alone, without inferring missing required work from unstated intent. A task that would make the implementer pick an approach, name an interface, or settle a policy is unfinished: resolve that decision here and write the resolution into the task.

**`investigate and fix` is not a task.** A task that bundles unfinished investigation with an unspecified repair has no boundary and no completion condition, so it is rejected outright. Investigation happens before the proposal exists, and only the resulting permanent transition becomes a task. The same rejection applies to `debug and correct`, `look into X and resolve it`, and any other task whose scope is decided only after it starts.

## Guardrails (Match Command Behavior)

- If `openspec/CONSTITUTION.md` exists, read it before drafting the proposal and treat it as higher-priority project law than proposal/spec deltas.
- Do not draft a proposal that violates `openspec/CONSTITUTION.md` unless the proposal explicitly changes that constitution first.
- Favor straightforward, minimal implementations first and add complexity only when it is requested or clearly required.
- Keep changes tightly scoped to the requested outcome.
- Default to proposal splitting: when requirements can be decomposed into independent scopes, create separate change proposals.
- If uncertain whether to split, prefer splitting unless the scopes are tightly coupled and must ship together to preserve correctness.
- For each split proposal, use a distinct verb-led `change-id` and keep `proposal.md`, `tasks.md`, and `design.md` (when needed) scoped to that proposal only.
- When multiple proposals are created, explicitly document dependency/sequence relationships and parallelizability in the final user-facing summary.
- Before asking clarifying questions, proactively gather context from the current session and repository, and treat that gathered context as the default premise for the proposal.
- Start the user-facing response with a short `Premise / Context` section summarizing the goals, constraints, and relevant repo architecture already discovered.
- Do not ask the user to choose or confirm the `change-id`; generate a concise unique verb-led slug yourself.
- If the request is sufficiently clear after context gathering, draft the proposal directly instead of forcing an extra clarification round.
- For implementation-oriented proposals, make tasks evidence-bearing: each behavior-changing task should name repository-verifiable code, tests, commands, or explicit manual checks.
- For behavior-changing proposals, require verification coverage planning per requirement/task so verification ownership is explicit at proposal time.
- Write tasks around required behavior, not artifact existence. Avoid task sets that can look complete when only contracts, docs, or placeholder wiring exist.
- Separate responsibility-definition/documentation tasks from runtime wiring/integration tasks so the proposal cannot be marked done without code paths, state changes, side effects, or externally visible behavior being connected.
- Treat every user-visible requirement, dependency, migration, verification activity, and non-goal as something that must be explicitly represented in the proposal artifacts, not left implicit.
- Every implementation-facing task must have an explicit completion condition so a later agent can determine done/not-done from repository evidence instead of judgment calls.
- For each major behavior-changing requirement, include at least one verification path that would fail if the implementation were stubbed, no-op, or returning dummy data.
- When the change exposes a CLI, API, workflow, job, or background process, include minimal execution verification covering a typical success path plus safe-mode / side-effect suppression / error handling where relevant.
- Prefer warnings over silent acceptance when tasks are dominated by “define / document / describe” language or when runtime behavior is claimed without runnable verification.
- Before choosing `spec-only`, explicitly classify the user's desired artifact as one of: `spec/documentation`, `implementation`, or `both`.
- Use the standard verification vocabulary for ownership planning: `unit`, `integration`, `e2e`, `manual`, `benchmark`, `not-testable`.
- Treat `manual` and `benchmark` as intentional coverage (not missing unit tests) when they fit the requirement better.
- If the user's request is phrased as a concrete product, UI, API, route, form, workflow, or behavior change, default to `implementation` or `hybrid`, not `spec-only`, unless the user explicitly asks for spec-only or documentation-only work.
- If canonical specs already describe the requested behavior but the codebase still lacks implementation, do NOT create another `spec-only` proposal unless the user explicitly asks to update spec artifacts only.
- When selecting `spec-only`, explicitly state in the user-facing response that the proposal will NOT implement the feature and will only update tracked specification artifacts.

## When to Use This Skill

Trigger this skill when users request:

- "Create a proposal for..."
- "Draft a change proposal"
- "Propose a new feature"
- "Document this change as a proposal"
- Any request to create structured change documentation

## Key Characteristics

**Human-Interactive Mode**:

- Ask clarifying questions to understand requirements
- Guide users through proposal structure
- Discuss design decisions and trade-offs
- Iterate on requirements based on feedback
- Present proposals for user review and approval

**Not for Orchestration**: This skill is designed for direct human interaction, not automated orchestration.

## Proposal Structure

A Conflux proposal consists of:

```
openspec/changes/<change-id>/
├── proposal.md          # Change description and context
├── tasks.md             # Implementation task checklist
├── design.md            # Architecture and design (optional)
└── specs/               # Spec deltas
    └── <capability>/
        └── spec.md      # Requirement specifications
```

## Interactive Workflow

### 1. Understand the Change

First, gather available context proactively before asking questions.

**Context to gather first**:

- Prior user messages, goals, and constraints from the current session
- Repository-specific instructions such as `AGENTS.md` when present
- Existing related specs, archived changes, and relevant source/test modules
- Existing architecture or workflow boundaries already mentioned in the conversation

**Begin the user-facing proposal response with**:

- `Premise / Context`
- 3-6 concise bullets summarizing the gathered facts that will shape the proposal
- `Requested Artifact` with one of: `spec/documentation`, `implementation`, or `both`

If the inferred requested artifact is `implementation` or `both`, do not silently downgrade the proposal to `spec-only`.

**Ask questions only if still necessary after context gathering**:

- What problem does this solve?
- What are the acceptance criteria?
- Are there any constraints or dependencies?
- What is the scope (minimal vs. comprehensive)?
- What required work would make the request incomplete if omitted from the proposal?

If the answer is already inferable from the repository and current conversation, skip the question and proceed.

Before proceeding, explicitly normalize the request into a completeness checklist covering:

- user-facing outcomes that must become true
- repository areas likely requiring change
- required verification proving each outcome
- dependencies, migrations, rollout concerns, and follow-up work that must be represented
- explicit non-goals / out-of-scope items to prevent accidental under-specification

**Research existing code**:

```bash
# Review existing specs
cflx openspec list --specs

# Check related code
rg "<keyword>"
ls <relevant-directory>
```

**Strict validation note (common gotcha)**:

- In strict mode, include at least one spec delta under `openspec/changes/<id>/specs/<capability>/spec.md`.
- Before writing `## MODIFIED Requirements` or `## REMOVED Requirements`, inspect the matching canonical `openspec/specs/<capability>/spec.md` and copy an existing `### Requirement:` heading exactly. These sections target existing canonical requirement identities; strict validation fails when the canonical heading is absent.
- Use `## ADDED Requirements` when no matching canonical `### Requirement:` heading exists yet.
- For bugfix-only proposals (no intended new behavior), add a minimal `## MODIFIED Requirements` delta only when the target requirement heading already exists in the canonical spec; otherwise use `## ADDED Requirements` for the new tracked behavior.

### 2. Classify Change Type

Every proposal MUST include a `Change Type` field in `proposal.md`. First classify the user's desired artifact:

- `spec/documentation`: the user explicitly wants proposal, spec, documentation, archive-readiness, or canonical requirement updates
- `implementation`: the user wants code, UI, API wiring, routes, forms, tests, runtime behavior, or removal of existing behavior in the product
- `both`: the user wants both spec and implementation to move together

Use this decision table:

| Type             | When to use                                                                                                                                                                                            |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `spec-only`      | The user's primary requested artifact is a canonical spec or documentation update. No new runtime code, CLI wiring, UI work, or tests are required. All tasks are specification or documentation work. |
| `implementation` | The proposal drives source code changes, tests, CLI wiring, UI work, or runtime behavior. Spec deltas describe what the code must satisfy.                                                             |
| `hybrid`         | The proposal combines spec authoring with implementation work — for example, adding a new spec capability and immediately implementing it in the same change.                                          |

**Strong defaults**:

- If the request is phrased as adding/removing/fixing a concrete product behavior, UI, page, route, form, API integration, workflow, or runtime feature, treat it as `implementation` by default.
- If existing canonical specs already describe the target behavior and the repo still lacks implementation, choose `implementation`, not `spec-only`, unless the user explicitly asks for spec cleanup only.
- Never choose `spec-only` merely because a backend surface already exists; if user-visible behavior is still missing from code, the default remains `implementation`.

**When to split instead of using `hybrid`**:

- If the spec authoring and the implementation can be reviewed and deployed independently, split into two proposals.
- Use `hybrid` only when spec and code must ship atomically to preserve correctness.

When drafting `proposal.md`, prefer YAML frontmatter at the top for machine-readable metadata:

```yaml
---
change_type: spec-only # or: implementation | hybrid
priority: medium # high | medium | low
dependencies: [] # optional change-id list; overrides body `## Dependencies`
references: [] # optional string list of related files/specs/changes
verifications:
  - id: local-tests
    requirement: Repository behavior covered before integration
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: path/to/tracked-test-or-script
    evidence: local test result or repository evidence location
    rerun: concrete local rerun command
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: deployed-smoke
    requirement: Operational outcome verified after integration
    phase: post-integration
    owner: repository-automation
    trigger: default-branch-integration
    automation: path/to/tracked-workflow-or-script
    evidence: expected published evidence location
    rerun: concrete recovery or rerun action
    prerequisites: []
    execution_class: deployed-service
    completion_role: operational-observation
---
```

Every implementation or hybrid proposal MUST declare at least one complete `pre-integration` verification. Every claimed post-integration outcome MUST use a complete `post-integration` declaration. `id`, `requirement`, `phase`, `owner`, `trigger`, `automation`, `evidence`, `rerun`, and `prerequisites` are required. `pre-integration` requires `owner: conflux-acceptance`; `post-integration` requires `owner: repository-automation`. `automation` must be a safe repository-relative tracked regular file.

Classify outcomes structurally. Do not put deployment URLs, external HTTP/API checks, or results available only after default-branch integration into pre-integration checkbox acceptance conditions. Record those as post-integration automation with evidence publication and rerun ownership.

**Verification execution class and completion role**

The `verifications` block is the only structured truth source for completion gating. Do not invent a parallel completion-gate block.

- `execution_class` is one of `repository-local`, `repository-automation`, `deployed-service`, `physical-device`, `external-approval`, `credentialed-external`.
- `completion_role` is one of `change-blocking`, `operational-observation`.
- `completion_role: change-blocking` requires `phase: pre-integration` **and** `execution_class: repository-local`. Nothing else may block Conflux acceptance, archive, or merge.
- Every `post-integration` declaration, and every non-local execution class, MUST be `completion_role: operational-observation`.
- Declare `execution_class` and `completion_role` together; declaring only one fails validation.
- Legacy declarations without the two fields stay valid during migration and receive migration warnings.

Every checkbox in an active implementation task section MUST reference a declared `change-blocking` verification with `verification-id: <id>`:

```markdown
- [ ] Implement the parser (verification: unit - cargo test parser; verification-id: local-tests)
```

Referencing an undeclared ID, or an `operational-observation` ID, fails validation. If an outcome cannot be produced locally before integration, it does not belong to an active checkbox: move it to `## Future Work` or to a separate release-observation change. Prose alone never turns a manual or external gate into a legitimate completion condition.

Keep the human-readable line near the top as well for backward-compatible readability:

```markdown
**Change Type**: spec-only <!-- or: implementation | hybrid -->
```

### 3. Evaluate Split Boundaries (Default: Split)

Before writing anything, evaluate whether the request should be split into multiple independent change proposals.

**Default rule**: if scopes are independent or weakly coupled, split into separate `openspec/changes/<change-id>/` proposals.

**Keep as a single proposal only when**:

- The scopes are tightly coupled and must ship atomically to preserve correctness.
- The acceptance criteria cannot be verified independently.

When keeping a single proposal despite multiple scopes, explicitly record the rationale in `proposal.md` or `design.md`.

### 3. Decide Dependency Eligibility

`dependencies` are hard implementation-order gates: a dependent change stays blocked until every dependency is archived and integrated into the base. Use them only for repository output the dependent change actually consumes.

**A hard dependency is eligible only when** the dependency's base-integrated repository output — concrete code, contract, schema, migration, configuration, or test surface — is required to implement the dependent change or to run its `pre-integration` verification. Record that consumed surface as the justification.

**Never use a hard dependency for**:

- Roadmap or backlog ordering
- MVP, milestone, or release phase boundaries
- Deployed-service checks or environment smoke tests
- Physical-device acceptance
- Human or external approval
- Credentialed access to an external system

Release sequence alone is never an acceptable justification. Those outcomes are `operational-observation` verifications, not implementation dependencies; making them block a hard dependency edge converts one operational release gate into a graph-wide development stop.

**Review reverse-dependency impact before adding an edge.** Before adding a dependency, or before adding a non-local verification to a change that already has dependents, list the direct and transitive downstream changes that would become blocked. If any of them only need repository-local contracts or implementation, the edge is wrong — remove it or split the target.

**Split repository-local implementation from release observation.** When one scope contains both locally completable implementation and independently executable non-local acceptance (physical device, deployed service, external approval, credentials), create two changes:

1. An implementation change that is completable from repository-local `change-blocking` evidence.
2. A release-observation change whose verifications are all `operational-observation`, with no active implementation checkboxes.

The release-observation change may depend on the implementation change, and release-observation changes may form ordered chains among themselves, because observations never block Conflux completion. Unrelated follow-on implementation changes must depend only on the repository outputs they consume — never on the release-observation change.

Strict validation rejects a repository-local implementation change that depends on a target declaring a non-local `change-blocking` verification, and names the dependent, target, verification ID, and remedy.

### 3. Generate Change ID

**Rules**:

- Verb-led (e.g., `add-auth`, `fix-validation`, `refactor-api`)
- Kebab-case (lowercase with hyphens)
- Must NOT include date prefixes or suffixes (forbidden: `2026-02-07-add-auth`, `add-auth-2026-02-07`)
- Concise but descriptive
- Unique within the project

**Execution rule**:

- Generate the `change-id` yourself.
- Do not ask the user to confirm or choose it.
- If a collision is possible, disambiguate automatically (for example by adding a short suffix).

**Present to user**: "Using change ID `<id>`."

### 4. Draft Proposal Content

Create `openspec/changes/<id>/proposal.md`:

**Required sections**:

- YAML frontmatter with `change_type`, `priority`, optional `dependencies`, optional `references`
- Title (H1)
- Problem/Context
- Proposed Solution
- Acceptance Criteria
- Explicit Completion Conditions
- Out of Scope (if applicable)

`Acceptance Criteria` must describe the user-visible or system-visible outcomes that need to hold when the change is done.
`Explicit Completion Conditions` must describe how a later agent/reviewer can tell the proposal has been fully implemented, including expected code paths, tests, commands, or artifacts.

**Ask for feedback**: "Here's the draft proposal. Would you like to adjust anything?"

### 5. Create Task Breakdown

Create `openspec/changes/<id>/tasks.md`:

**Task format for `implementation` or `hybrid` proposals**:

```markdown
## Implementation Tasks

- [ ] Task 1: Description (verification: how to verify completion)
- [ ] Task 2: Description (verification: ...)

## Future Work

- Items that require human action
- Items requiring external systems
- Long-wait verification tasks
```

**Task format for `spec-only` proposals** — use `## Specification Tasks` instead of `## Implementation Tasks`, and include a one-line expected canonical outcome for each delta:

```markdown
## Specification Tasks

- [ ] Promote `specs/capability-name/spec.md` delta to canonical spec
  - Expected canonical result: <what the canonical spec will contain after archive>
- [ ] Review and validate delta scenarios for completeness

## Future Work

- Human sign-off on canonical promotion
```

> **Note**: For `spec-only` proposals, each spec delta should include a short comment describing how the canonical spec changes after archive. This allows acceptance to evaluate archive-readiness without expecting runtime integration evidence.

**Guidelines**:

- Break into small, verifiable steps
- Do not add a checkbox whose only work is rerunning a repository-wide check that a tracked, unconditional commit hook already runs; see [Hook-Owned Repository-Wide Verification](#hook-owned-repository-wide-verification)
- Include verification methods
- Ensure the task list is complete enough that Conflux can choose execution order without depending on unstated human intent
- Represent every required implementation, integration, migration, verification, and documentation step needed to fully satisfy the request
- For each behavior-changing requirement/task, record planned verification ownership using one of: `unit`, `integration`, `e2e`, `manual`, `benchmark`, `not-testable`
- Start each behavior-bearing verification note with that ownership marker (for example `verification: integration - cargo test --test auth_flow`)
- When `manual` or `benchmark` is selected, state why it is intentional coverage and how reviewers will evaluate completion
- Specify integration/wiring tasks separately from responsibility-definition or documentation tasks
- Mark non-AI-executable tasks for Future Work
- Prefer concrete repository evidence in verification notes (source paths, test files, runnable commands, or explicit manual observation steps), not vague statements like "verify implementation works"
- Make task verification notes traceable as ownership pairs: implementation task + verification path (for example `verification: integration - tests/api/test_auth_flow.rs`)
- Every checkbox task must have a completion condition that can be evaluated from repo state, test results, generated artifacts, or an explicit manual observation
- For major requirements, at least one verification path must require real implementation behavior rather than spec text or placeholder wiring
- If a CLI/API/workflow/job/background process is in scope, include a minimal runnable verification for expected success behavior and relevant safe-mode / no-op / error conditions
- If omitting a task would leave some acceptance criterion unsatisfied, the task belongs in the proposal even if the implementation order is unknown

**External dependency policy (mock-first / verification-first)**:

- If a requirement cannot be verified locally without credentials or external systems, design mock/stub/fixture-based verification.
- Do not change production/runtime behavior to "use mocks"; mocks/stubs/fixtures are for tests and local verification.
- Only truly non-mockable dependencies (human decisions, real external systems, long-wait checks) go to Out of Scope / Future Work.

**Present to user**: "I've broken this down into X tasks. Do these cover everything?"

### 6. Design Documentation (Optional)

Create `openspec/changes/<id>/design.md` when:

- Change spans multiple systems
- Architectural decisions need documentation
- Trade-offs require explanation
- Complex implementation patterns

Design documentation is strongly recommended when the proposal introduces orchestration, durable state, cross-service coordination, background workers, adapters, or other multi-layer behavior.

**Ask**: "Should we document the design decisions in detail?"

### 7. Write Spec Deltas

Create `openspec/changes/<id>/specs/<capability>/spec.md`:

**Format**:

```markdown
## ADDED Requirements

### Requirement: <requirement-name>

<Description>

#### Scenario: <scenario-name>

**Given**: <preconditions>
**When**: <action>
**Then**: <expected-outcome>

## MODIFIED Requirements

### Requirement: <existing-requirement-name>

<Updated description>

#### Scenario: <scenario-name>

...

## REMOVED Requirements

### Requirement: <deprecated-requirement-name>

<Reason for removal>
```

**Critical rules**:

- Each requirement must have at least one scenario
- Use ADDED/MODIFIED/REMOVED sections
- Before selecting MODIFIED or REMOVED, open `openspec/specs/<capability>/spec.md` and verify the canonical `### Requirement:` heading exists; copy that heading exactly into the delta.
- If the canonical heading does not exist, use ADDED instead of MODIFIED/REMOVED.
- Be specific and testable

**Discuss with user**: "Should we add these requirements to the spec?"

### 8. Validate Proposal

Run validation after authoring proposal, tasks, and spec deltas:

```bash
cflx openspec validate <id> --strict
```

Strict validation checks MODIFIED/REMOVED target headings against canonical specs, so fix any missing-target diagnostics before handoff.

**If validation fails**:

- Show errors to user
- Discuss fixes
- Apply corrections
- Re-validate

**Present results**: "Validation passed! The proposal is ready for review."

### 9. Final Review

Present complete proposal to user:

- Show directory structure
- Summarize key points
- Highlight task count
- Confirm readiness

When the proposal was split into multiple independent change proposals, always present a proposal index:

```
- change-id
  - one-line objective
  - dependency/sequence (if any)
  - whether it can be implemented in parallel
```

**Ask**: "The proposal is complete. Would you like to proceed with implementation, or make any final adjustments?"

## Mock-First External Dependencies

When designing tasks, follow mock-first approach:

**Prefer**:

- Mock/stub/fixture implementations
- Test doubles for external APIs
- Local verification without credentials

**Avoid**:

- Blocking on missing API keys
- Requiring real external services
- Deferring mockable dependencies

**Discuss with user**: "For the external API integration, should we use mocks for testing, or do you have test credentials available?"

## Hook-Owned Repository-Wide Verification

Conflux creates the final Apply commit itself, with repository hooks enabled. A
repository-wide format, lint, fast-test, or generated-artifact check that a
tracked commit hook already runs therefore executes on every change with no task
of its own. Adding a checkbox whose only work is rerunning that same command
duplicates it, and it pushes the Apply agent into starting a long validation
command it may return from before it finishes.

**Inspect the tracked hook configuration before delegating anything.** Read the
repository's own committed hook definition — for example `.pre-commit-config.yaml`,
`lefthook.yml`, `.husky/`, or the tracked directory named by `core.hooksPath`.
Untracked local hooks and developer machine setup are not evidence.

**Delegate a repository-wide gate to the hook only when the tracked definition
proves it runs unconditionally**, including on the clean-tree amend path. Normal
Apply finalization amends a WIP commit whose staged diff can be empty, so a hook
that only receives or filters staged filenames may observe nothing and skip
validation entirely.

- Qualifies: a pre-commit entry with `always_run: true` and
  `pass_filenames: false`, or an equivalent hook script that runs the whole
  repository command regardless of what is staged.
- Does not qualify: `lint-staged`, `pass_filenames: true`, `--staged`,
  changed-file globs, `files:`/`types:` filters, or any hook whose command line
  is built from the staged file list. Treat these as **staged-file-only hooks**
  and keep an explicit executable verification path in the proposal.

**What never moves into the hook**:

- Requirement-specific test implementation and its verification note. These stay
  attached to the implementation task that introduces the behavior.
- Heavy, E2E, networked, credentialed, long-running, and post-integration
  checks. These keep an explicit verification owner (`verifications:` entry with
  its own `phase`, `owner`, and `rerun`) outside pre-commit. Do not silently move
  them into pre-commit, and do not omit them because pre-commit exists.

When a gate is delegated, say so once in the proposal (for example under
`Out of Scope` or the verification plan) naming the tracked hook that owns it,
so a reviewer can check the same evidence.

## Apply-Blocking Verification Must Be Bounded and Repository-Local

An active checkbox task is work Apply must finish inside one bounded invocation.
Conflux enforces an absolute runtime limit on that invocation
(`command_max_runtime_secs`, default 10800s / 3 hours), so a checkbox that requires a gate
Apply cannot finish does not produce late evidence — it produces a terminated
command and no evidence at all.

**Keep Apply-blocking verification bounded to one direct execution by default.**
Name exactly one rerun command per verification. Never write a checkbox whose
only purpose is repeated execution of the same command ("run the suite three
times", "confirm stability", "verify it is not flaky"). Stability repetition is
not verification planning; it is an unbounded loop with a task number.

**These gates are NOT Apply-blocking checkbox work**, because none of them can be
guaranteed to complete inside one bounded repository-local invocation:

| Gate | Assign ownership to |
| --- | --- |
| Docker / container orchestration suites | repository automation (CI) or Acceptance |
| Database or data-migration suites needing a real server | repository automation or Acceptance |
| `heavy`-marked or full repository-wide long-running suites | repository automation or Acceptance |
| Credentialed or networked external-service checks | Acceptance or operational observation |
| Deployed-service, staging, or production checks | operational observation (`post-integration`) |
| Physical-device or hardware-in-the-loop checks | manual, with a named owner |
| External approval or human sign-off | narrative `## Future Work` (no checkbox) |

Give each one a structured `verifications:` entry with its own `phase`, `owner`,
`trigger`, `rerun`, `execution_class`, and `completion_role`. Do not omit it, and
do not bury it in task prose where it would silently become Apply's problem.

**Exception — a bounded repository-local path may block completion.** When the
same requirement can be proven by a local fixture, fake, in-memory double, or
testcontainer-free harness that completes in one direct command, declare *that*
as `pre-integration`, `repository-local`, and `change-blocking`, and attach it to
the implementation task. The heavy suite keeps its own separate non-blocking
ownership. This is the preferred shape: requirement-specific bounded proof
blocks the change, and the broad suite guards the integration. The bounded path
is an exception for the *requirement*, never for the *command form*: a bounded
repository-local declaration whose `evidence` or `rerun` still names one of the
warned forms below is flagged by native validation, and no wording removes the
finding.

**Never hide a non-local outcome in task prose.** If the outcome needs
credentials, a deployment, hardware, or an approval, it does not become
Apply-blocking by being phrased as "verify that ...". Move it to its structured
verification owner or to narrative Future Work.

## Structured Frontmatter Is the Only Command Authority

Native strict validation reads commands from exactly two places:
`verifications[].evidence` and `verifications[].rerun`. Nothing else is a
command-authority source.

**Task prose is never parsed for commands.** A task note links a checkbox to its
gate with `verification-id: <id>`; it does not duplicate, override, or weaken the
declared command. Validation derives no cohesion rule from task-note ownership
markers, and it produces no heaviness finding from a word or a command that
appears only in task text. Several checkboxes may share one change-blocking
`verification-id:` — that is the normal shape for coupled implementation and
regression work:

```markdown
- [ ] Add the token matcher (verification: unit - `cargo test openspec_cmd --lib`; verification-id: proposal-gate-tests)
- [ ] Add its regression tests (verification: unit - `cargo test openspec_cmd --lib`; verification-id: proposal-gate-tests)
```

**Validation constrains what you declare, not what a session runs.** The
validator never executes `evidence` or `rerun`, and it cannot stop an AI session
from independently choosing a heavy command. It removes the *declared
authorization* for one; keeping the actual execution bounded stays the job of
this guidance and of the Apply skill's bounded-verification discipline.

## Heavyweight Command Forms on a Change-Blocking Gate

**During migration these are warnings.** A match is reported by strict
validation with the verification ID and the matched form, and it does **not**
fail `cflx openspec validate <id> --archive-gate`. A later reviewed proposal may
promote proven classes to errors once migration evidence exists — so treat a
warning as work to do now, not as a finding to ignore.

The check applies to both declared command forms, `evidence` and `rerun`, so a
heavy `evidence` is still reported behind a focused `rerun`. Warned forms:

| Warned form | Examples |
| --- | --- |
| Container orchestration | `docker compose`, `docker-compose`, `docker run`, `docker swarm`, `podman`, `kubectl` |
| Architecture emulation | `qemu-system-*`, `cross` |
| Benchmark | `cargo bench` |
| Broad selector | `--workspace`, `--all-features`, `--ignored`, `--include-ignored`, `--features heavy`, `--exhaustive` |
| Structural repetition | `seq`, `xargs` |

**Matching is exact, by whole token.** Tokens are compared case-insensitively
after Markdown backticks are removed and whitespace is folded; multi-token forms
such as `docker compose` and `--features heavy` require adjacent tokens, and
`qemu-system-*` is the one executable-prefix form. Substring containment is never
a match, so these stay valid:

- `docker build .` — a bounded image build is explicitly permitted and is not
  container orchestration.
- `cargo test full_pipeline_smoke --lib`, `cargo test benchmark_parser_units
  --lib`, `cargo test exhaustive_token_matching --lib` — `full`, `heavy`,
  `benchmark`, and `exhaustive` inside a longer token are not selectors.

Structural repetition is detected from `seq` and `xargs` in the declared command,
including inside shell syntax such as `for i in $(seq 3); do cargo test; done`.
Prose like "run it three times" is not parsed at all — but do not write that task
either; see the bounded-execution rule above.

Rewrite a warned gate as bounded proof plus separately owned broad verification:

```yaml
verifications:
  # Bounded requirement-specific proof: this one blocks the change.
  - id: parser-gate-tests
    requirement: The parser warns about declared heavyweight change-blocking commands
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/openspec_cmd/validation.rs
    evidence: cargo test openspec_cmd --lib
    rerun: cargo test openspec_cmd --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  # Broad execution: observed, never blocking.
  - id: container-integration-sweep
    requirement: The packaged image still runs the full suite
    phase: post-integration
    owner: repository-automation
    trigger: merge-to-main
    automation: .github/workflows/integration.yml
    evidence: docker compose run --rm suite
    rerun: docker compose run --rm suite
    prerequisites: []
    execution_class: repository-automation
    completion_role: operational-observation
```

Cache-busting forms such as `-count=1` are bounded and are not matched at all.

## Task Classification

**AI-Executable Tasks** (include with checkbox):

- Code implementation
- Unit/integration tests
- Documentation updates
- Linting/formatting
- Local verification

**Future Work Tasks** (no checkbox):

- Manual approval required
- Human decision-making
- External system deployment
- Long-wait verification (>1 day)

**Ask user**: "Are there any tasks that require manual review or external approvals?"

## Question Examples

### Understanding Requirements

- "What's the primary user need this addresses?"
- "Are there any security or performance requirements?"
- "What's the expected timeline for this change?"

### Clarifying Scope

- "Should this include error handling for edge cases?"
- "Do we need backward compatibility?"
- "Are there related features we should consider?"

### Design Decisions

- "Would you prefer approach A (simpler) or B (more flexible)?"
- "Should we optimize for performance or maintainability?"
- "Where should this integrate with the existing system?"

### Verification

- "How should we verify this is working correctly?"
- "What would constitute a successful implementation?"
- "Are there specific test scenarios we should cover?"

## Built-in Tools

Use `cflx openspec` for all Conflux OpenSpec operations:

```bash
# List existing changes
cflx openspec list

# List specs
cflx openspec list --specs

# Show change details
cflx openspec show <id>

# Validate proposal
cflx openspec validate <id> --strict

# Validate proposal with implementation-evidence warnings
cflx openspec validate <id> --strict --evidence warn

# Validate proposal with implementation-evidence errors
cflx openspec validate <id> --strict --evidence error

# Recommended authoring check for behavior-changing proposals
cflx openspec validate <id> --strict --evidence warn
# Resolve warnings about missing ownership, artifact-heavy tasks,
# absent runnable verification, or executable surfaces lacking run checks

# Archive-equivalent readiness check for final validation sections
cflx openspec validate <id> --archive-gate
```

Final validation guidance must be written outside checkbox task lists, for example:

```markdown
## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate <id> --archive-gate`
```

`Final Validation`, `Implementation Blocker #N`, `Future Work`, `Out of Scope`, `Notes`, and `Acceptance Notes` are narrative non-task sections: plain prose and non-checkbox `- ` bullets belong there, checkboxes do not.

Active task sections are the mirror image: every top-level `- ` or `* ` line must be a checkbox task. Never plan a bare `- evidence: ...` or `- note: ...` bullet inside an active task section — native validation rejects it with `Possible task without checkbox`, and Conflux keeps the change in apply until it is fixed.

## Best Practices

### Communication

- Ask one question at a time (avoid overwhelming)
- Summarize understanding before proceeding
- Present options with recommendations
- Confirm major decisions
- Prefer making a reasonable default proposal after context gathering over repeatedly blocking on optional clarification
- When you must ask a question, state the recommended default and what would change based on the answer

### Proposal Quality

- Keep scope focused and minimal
- Break large changes into multiple proposals
- Include clear acceptance criteria
- Add explicit completion conditions so another agent can decide done/not-done without inferring missing intent
- For behavior-changing proposals, plan verification ownership per requirement/task using `unit` / `integration` / `e2e` / `manual` / `benchmark` / `not-testable`
- Treat `manual` and `benchmark` as valid intentional coverage when justified by requirement characteristics
- Specify verification for each task
- Make implementation-facing tasks hard to mark complete without source/test evidence
- Ensure tasks guidance keeps ownership traceable by pairing each implementation task with its verification path
- Do not create final OpenSpec validation as a checkbox implementation task. Final validation belongs in a non-checkbox `## Final Validation` section because archive validation is the authoritative gate.
- Check for proposal completeness by asking: "If Conflux executed these tasks in its own chosen order, would the full user request still be satisfied without any hidden assumptions?"

### User Experience

- Be responsive to feedback
- Iterate on content based on input
- Present information clearly
- Provide helpful suggestions

## Common Patterns

### Feature Addition

1. Understand user need
2. Review existing architecture
3. Propose minimal viable solution
4. Break into implementation tasks
5. Add spec requirements

### Bug Fix

1. Understand the bug behavior
2. Identify root cause area
3. Propose fix approach
4. Include regression tests
5. Add MODIFIED spec if behavior changes

### Refactoring

1. Clarify refactoring goals
2. Identify affected components
3. Plan incremental steps
4. Ensure test coverage
5. Document design decisions

## Troubleshooting

### User Unclear About Requirements

- Ask more specific questions
- Provide examples or options
- Start with minimal scope
- Plan for iteration

### Complex Dependencies

- Discuss mock-first approach
- Identify what's truly non-mockable
- Plan integration points
- Document assumptions
- Confirm each hard dependency names the repository output the dependent change consumes
- Move release sequencing, deployment checks, device acceptance, and approvals into `operational-observation` verifications instead of dependency edges
- Re-check direct and transitive dependents before adding an edge or a non-local verification

### Large Scope

- Suggest breaking into multiple proposals
- Identify core vs. nice-to-have
- Prioritize with user
- Plan phased approach

## Reference Files

No additional reference files.

## Example Interaction

```
User: "I want to add user authentication"

Agent: "I'd like to understand the authentication requirements better:
1. What type of authentication? (JWT, session-based, OAuth?)
2. What user actions need to be protected?
3. Do you have an existing user database or need to create one?"

User: "JWT-based, protect API endpoints, we have a users table"

Agent: "Got it. I suggest the change ID `add-jwt-auth`. I'll create a proposal with:
- JWT token generation and validation
- Middleware for protected routes
- Login/logout endpoints

Does this scope look correct?"

User: "Yes, please proceed"

Agent: [Creates proposal.md, tasks.md, specs/]
"Here's the task breakdown:
1. Implement JWT utility functions
2. Create auth middleware
3. Add login endpoint
4. Add logout endpoint
5. Protect existing API routes
6. Add unit tests
7. Add integration tests

Total: 7 tasks. Does this cover everything?"

User: "Add password reset too"

Agent: "I'll add password reset. This requires email sending - should we:
A) Mock email for testing (recommended)
B) Use a real email service (needs credentials)
?"

User: "Mock for now"

Agent: [Updates proposal]
"Added 2 more tasks for password reset with mocked email.
Running validation... ✓ Validation passed!

The proposal is ready. Would you like to start implementation?"
```
