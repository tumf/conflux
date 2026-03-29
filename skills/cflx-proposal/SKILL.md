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

## Guardrails (Match Command Behavior)

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
- For implementation-oriented proposals, make tasks evidence-bearing: each behavior-changing task should name repository-verifiable code, tests, or commands.

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
- Repository-specific instructions such as `AGENTS.md` or `openspec/AGENTS.md` when present
- Existing related specs, archived changes, and relevant source/test modules
- Existing architecture or workflow boundaries already mentioned in the conversation

**Begin the user-facing proposal response with**:
- `Premise / Context`
- 3-6 concise bullets summarizing the gathered facts that will shape the proposal

**Ask questions only if still necessary after context gathering**:
- What problem does this solve?
- What are the acceptance criteria?
- Are there any constraints or dependencies?
- What is the scope (minimal vs. comprehensive)?

If the answer is already inferable from the repository and current conversation, skip the question and proceed.

**Research existing code**:
```bash
# Review existing specs
 python3 "<SKILL_ROOT>/scripts/cflx.py" list --specs

# Check related code
rg "<keyword>"
ls <relevant-directory>
```

**Strict validation note (common gotcha)**:
- In strict mode, include at least one spec delta under `openspec/changes/<id>/specs/<capability>/spec.md`.
- For bugfix-only proposals (no intended new behavior), add a minimal `## MODIFIED Requirements` delta with at least one `### Requirement:` and one `#### Scenario:`.

### 2. Classify Change Type

Every proposal MUST include a `Change Type` field in `proposal.md`. Use this decision table:

| Type | When to use |
|------|-------------|
| `spec-only` | The proposal's primary output is a canonical spec update. No new runtime code, CLI wiring, or tests are required. All tasks are specification or documentation work. |
| `implementation` | The proposal drives source code changes, tests, CLI wiring, or runtime behavior. Spec deltas describe what the code must satisfy. |
| `hybrid` | The proposal combines spec authoring with implementation work — for example, adding a new spec capability and immediately implementing it in the same change. |

**When to split instead of using `hybrid`**:
- If the spec authoring and the implementation can be reviewed and deployed independently, split into two proposals.
- Use `hybrid` only when spec and code must ship atomically to preserve correctness.

When drafting `proposal.md`, prefer YAML frontmatter at the top for machine-readable metadata:

```yaml
---
change_type: spec-only   # or: implementation | hybrid
priority: medium         # high | medium | low
dependencies: []         # optional change-id list; overrides body `## Dependencies`
references: []           # optional string list of related files/specs/changes
---
```

Keep the human-readable line near the top as well for backward-compatible readability:

```markdown
**Change Type**: spec-only   <!-- or: implementation | hybrid -->
```

### 3. Evaluate Split Boundaries (Default: Split)

Before writing anything, evaluate whether the request should be split into multiple independent change proposals.

**Default rule**: if scopes are independent or weakly coupled, split into separate `openspec/changes/<change-id>/` proposals.

**Keep as a single proposal only when**:
- The scopes are tightly coupled and must ship atomically to preserve correctness.
- The acceptance criteria cannot be verified independently.

When keeping a single proposal despite multiple scopes, explicitly record the rationale in `proposal.md` or `design.md`.

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
- Out of Scope (if applicable)

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
- Include verification methods
- Specify integration/wiring tasks
- Mark non-AI-executable tasks for Future Work
- Prefer concrete repository evidence in verification notes (source paths, test files, or runnable commands), not vague statements like "verify implementation works"

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
- Be specific and testable

**Discuss with user**: "Should we add these requirements to the spec?"

### 8. Validate Proposal

Run validation:
```bash
 python3 "<SKILL_ROOT>/scripts/cflx.py" validate <id> --strict
```

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

Use `python3 "<SKILL_ROOT>/scripts/cflx.py"` for all Conflux operations (`<SKILL_ROOT>` is a placeholder for the skill's base directory path):

```bash
# List existing changes
python3 "<SKILL_ROOT>/scripts/cflx.py" list

# List specs
python3 "<SKILL_ROOT>/scripts/cflx.py" list --specs

# Show change details
python3 "<SKILL_ROOT>/scripts/cflx.py" show <id>

# Validate proposal
python3 "<SKILL_ROOT>/scripts/cflx.py" validate <id> --strict

# Validate proposal with implementation-evidence warnings
python3 "<SKILL_ROOT>/scripts/cflx.py" validate <id> --strict --evidence warn

# Validate proposal with implementation-evidence errors
python3 "<SKILL_ROOT>/scripts/cflx.py" validate <id> --strict --evidence error
```

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
- Specify verification for each task
- Make implementation-facing tasks hard to mark complete without source/test evidence

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
