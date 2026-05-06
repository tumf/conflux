# CFLX Skills

Conflux workflow management skills for AI coding assistants.

## Overview

Conflux uses a **router + per-operation skill** architecture. New orchestrator prompts load dedicated operation-specific skills directly, while `cflx-workflow` remains as a backward-compatible router for legacy prompts.

```
[Human] → cflx-proposal → Proposal Creation (interactive)
                ↓
        [Committed change on base branch]
                ↓
           cflx-run → `cflx run` orchestration
                ↓
        ┌─────────────────────────────────────┐
        │  Orchestrator loads dedicated skills │
        ├─────────────────────────────────────┤
        │  cflx-analyze  → Dependency analysis│
        │  cflx-apply    → Implementation     │
        │  cflx-rejecting→ Rejection review   │
        │  cflx-cleanup-review → Cleanup      │
        │  cflx-accept   → Acceptance review  │
        │  cflx-accept-with-speca → SPECA lens│
        │  cflx-archive  → Finalization       │
        │  cflx-resolve  → Conflict resolution│
        └─────────────────────────────────────┘
```

## Skills

### cflx-proposal

**Purpose**: Create structured change proposals through interactive conversation with users.

**Characteristics**:
- Human-interactive mode
- Asks clarifying questions
- Guides users through proposal structure

### cflx-run

**Purpose**: Prepare a clean base branch and run `cflx run` for committed OpenSpec changes.

**Characteristics**:
- Human-invoked operational mode
- Verifies clean working tree and base branch
- Runs Conflux orchestration and reviews the merge result

### Dedicated Workflow Skills

Most of these skills are loaded directly by the orchestrator. `cflx-rejection-guide` is the operator-facing exception for rejected vs blocked guidance.

| Skill | Operation | Purpose |
|-------|-----------|---------|
| `cflx-analyze` | analyze | Dependency analysis and change selection |
| `cflx-apply` | apply | Implement approved changes |
| `cflx-rejecting` | rejecting | Review rejection proposals |
| `cflx-rejection-guide` | operator guide | Explain how to handle rejected vs blocked changes |
| `cflx-cleanup-review` | cleanup-review | Post-apply worktree cleanup |
| `cflx-accept` | accept | Acceptance review (operation identity) |
| `cflx-accept-with-speca` | accept | Acceptance review with SPECA-style property/proof-attempt lens |
| `cflx-archive` | archive | Finalize deployed changes |
| `cflx-resolve` | resolve | Merge conflict resolution |

The orchestrator-loaded operation skills are autonomous (cannot ask questions) and are called by the orchestration system. `cflx-rejection-guide` is intended for direct human/operator guidance.

When configurable operation skills are available, select the SPECA acceptance lens with:

```jsonc
{
  "accept_skill": "cflx-accept-with-speca"
}
```

This changes only the acceptance prompt prelude (`load skills: cflx-accept-with-speca`); it does not require changing `acceptance_command` or the acceptance verdict parser.

### cflx-workflow (Compatibility Router)

**Purpose**: Backward-compatible router for legacy prompts that use `load skills: cflx-workflow`.

**Characteristics**:
- Self-contained: provides legacy-equivalent guidance for apply / rejecting / cleanup-review / accept / archive without requiring additional skill loads
- Does not require cross-skill auxiliary file access
- New orchestrator prompts should use dedicated operation-specific skills instead

## Installation

```bash
cflx install-skills
```

This installs all bundled skills:
- `cflx-proposal` - For interactive proposal creation
- `cflx-run` - For executing `cflx run` from a clean base branch
- `cflx-workflow` - Compatibility router for legacy prompts
- `cflx-analyze` - Dependency analysis
- `cflx-apply` - Change implementation
- `cflx-rejecting` - Rejection review
- `cflx-rejection-guide` - Operator guidance for rejected vs blocked changes
- `cflx-cleanup-review` - Post-apply cleanup
- `cflx-accept` - Acceptance review identity
- `cflx-accept-with-speca` - Acceptance review with SPECA-style property checks
- `cflx-archive` - Change archival
- `cflx-resolve` - Conflict resolution

## Requirements

- **cflx binary**: The `cflx` binary provides native `cflx openspec` subcommands for all OpenSpec operations
- **Git**: For version control operations

## Built-in Tools

OpenSpec operations are provided natively by the `cflx` binary:

```bash
cflx openspec list                # List changes
cflx openspec list --specs        # List specs
cflx openspec show <id>           # Show change details
cflx openspec validate <id> --strict  # Validate change
cflx openspec archive <id> --yes  # Archive change
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

### Implementation Blocker Stalled Hold

- Apply can escalate `IMPLEMENTATION_BLOCKER` when implementation is truly impossible in current loop
- Accept treats a valid Implementation Blocker as a stalled acceptance hold; during the compatibility period it returns `ACCEPTANCE: GATED` / `{"acceptance":"gated"}` only as protocol handoff tokens with concrete blocker evidence (legacy `BLOCKED` is input compatibility only)

### Autonomous Execution (all operation-specific skills)

- No questions allowed during execution
- Make decisions based on available context
- Do not defer tasks based on difficulty

### Constitutional Priority

- If `openspec/CONSTITUTION.md` exists, read it before authoring, selecting, applying, accepting, archiving, resolving, or reviewing changes.
- Treat `openspec/CONSTITUTION.md` as higher-priority project law than proposal/spec deltas when they conflict.
- Do not author, approve, or implement changes that violate `openspec/CONSTITUTION.md` unless that constitution is explicitly changed first.

## License

MIT
