# CFLX Skills

Conflux workflow management skills for AI coding assistants.

## Overview

This repository contains three complementary skills for managing the Conflux (OpenSpec-based) change lifecycle:

```
[Human] → cflx-proposal → Proposal Creation (interactive)
                ↓
        [Committed change on base branch]
                ↓
           cflx-run → `cflx run` orchestration
                ↓
        cflx-workflow (apply) → Implementation (autonomous)
                ↓
        cflx-workflow (accept) → Verification (autonomous)
                ↓
        cflx-workflow (archive) → Finalization (autonomous)
```

## Skills

### cflx-proposal

**Purpose**: Create structured change proposals through interactive conversation with users.

**Characteristics**:
- Human-interactive mode
- Asks clarifying questions
- Guides users through proposal structure
- Iterates based on feedback

**Triggers**:
- "Create a proposal for..."
- "Draft a change proposal"
- "Propose a new feature"

### cflx-workflow

**Purpose**: Execute Conflux workflow operations autonomously without user interaction.

**Characteristics**:
- Called by orchestration system
- Cannot ask questions
- Makes autonomous decisions
- Three operations: apply, accept, archive

**Operations**:

| Operation | Purpose | Output |
|-----------|---------|--------|
| Apply | Implement approved changes | Completed tasks + code |
| Accept | Verify implementation | PASS / FAIL / CONTINUE / BLOCKED |
| Archive | Finalize deployed changes | Archived change + updated specs |

### cflx-run

**Purpose**: Prepare a clean base branch and run `cflx run` for committed OpenSpec changes.

**Characteristics**:
- Human-invoked operational mode
- Verifies clean working tree and base branch
- Checks whether upstream sync is needed
- Runs Conflux orchestration and reviews the merge result

## Installation

```bash
npx skills add tumf/cflx-skills
```

This will install all three skills:
- `cflx-proposal` - For interactive proposal creation
- `cflx-run` - For executing `cflx run` from a clean base branch
- `cflx-workflow` - For autonomous workflow execution

## Requirements

- **cflx binary**: The `cflx` binary provides native `cflx openspec` subcommands for all OpenSpec operations
- **Git**: For version control operations

## Built-in Tools

OpenSpec operations are provided natively by the `cflx` binary:

```bash
# List changes
cflx openspec list

# List specs
cflx openspec list --specs

# Show change details
cflx openspec show <id>

# Validate change
cflx openspec validate <id> --strict

# Archive change
cflx openspec archive <id> --yes
```

## Directory Structure

```
openspec/
├── changes/
│   ├── <change-id>/
│   │   ├── proposal.md
│   │   ├── tasks.md
│   │   ├── design.md (optional)
│   │   └── specs/
│   │       └── <capability>/
│   │           └── spec.md
│   └── archive/
└── specs/
    └── <capability>/
        └── spec.md
```

## Key Principles

### Mock-First External Dependencies

- Mock/stub/fixture external dependencies for verification
- Do not block on missing API keys or credentials
- Only truly non-mockable dependencies go to Future Work

### Task Management

- Update `tasks.md` immediately after each task completion
- Active sections must have checkboxes (`- [ ]` or `- [x]`)
- Future Work sections must NOT have checkboxes

### Implementation Blocker Gate

- Apply can escalate `IMPLEMENTATION_BLOCKER` when implementation is truly impossible in current loop
- Accept can return `ACCEPTANCE: BLOCKED` only with concrete blocker evidence

### Autonomous Execution (cflx-workflow only)

- No questions allowed during execution
- Make decisions based on available context
- Do not defer tasks based on difficulty

## License

MIT
