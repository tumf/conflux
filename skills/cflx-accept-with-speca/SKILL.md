---
name: cflx-accept-with-speca
description: Conflux acceptance review with an additional SPECA-style property/proof-attempt lens. Preserves the .opencode/commands/cflx-accept.md verdict contract and cannot ask questions or request user input.
---

# Conflux Acceptance Review with SPECA Lens

Use this skill when the acceptance operation should add SPECA-style property review to the standard Conflux acceptance process.

**CRITICAL**: This skill CANNOT ask questions to users. Make autonomous acceptance judgments from repository evidence only.

## Purpose

This skill adds a property-oriented falsification pass to Conflux acceptance. It does not replace standard acceptance. The fixed acceptance checks, verdict workflow, and final machine-readable verdict format remain owned by `.opencode/commands/cflx-accept.md` and the standard `cflx-accept` acceptance contract.

If `openspec/CONSTITUTION.md` exists, read it before acceptance review and treat it as higher-priority project law than proposal/spec deltas when judging correctness.

## Operation Identity

- **Mode**: Acceptance review
- **Additional lens**: SPECA-style property derivation and proof/falsification attempt
- **Goal**: Verify that implementation evidence satisfies OpenSpec requirements, task claims, and derived properties
- **Output**: Use the existing Conflux acceptance verdict contract exactly as defined by `.opencode/commands/cflx-accept.md`

## Single-Source Verdict Constraint

`.opencode/commands/cflx-accept.md` is the single source of truth for fixed acceptance procedure, checklist ownership, retry semantics, and final verdict formatting. This skill MUST NOT redefine that protocol.

Use the standard Conflux acceptance outcomes only: `pass`, `fail`, `continue`, or `gated`. For blocking SPECA/property failures, return the standard JSON `fail` verdict with actionable `findings` under the command-template contract. Do not emit any SPECA-specific terminal marker or alternate verdict line.

## Official NyxFoundation/speca Runner Adapter (Optional)

Attempt the official NyxFoundation/speca runner when it is locally available and usable. This runner is a supporting proof/falsification helper only; it is not required for acceptance and it never replaces repository evidence or the final Conflux verdict contract.

### Workspace boundary and artifact locations

Keep all official SPECA runner artifacts outside the target Conflux worktree by default:

- SPECA checkout/cache: `~/tmp/speca`
- Generated Conflux/OpenSpec input bundle: `~/tmp/speca-conflux-input/<change-id>/`
- Official SPECA outputs and logs: `~/tmp/speca-conflux-output/<change-id>/` or a documented output directory inside the external `~/tmp/speca` checkout

Do not clone NyxFoundation/speca, write generated inputs, store runner outputs, or place runner logs inside tracked Conflux paths unless a separate implementation task explicitly asks for a tracked fixture. Deleting the out-of-worktree SPECA input/output/log/cache directories must not change the next Conflux action for the same workspace file state and git state.

### Prerequisite checks before running

Before launching setup or execution, inspect the installed SPECA checkout and verify:

- `uv` is installed and available on `PATH`.
- `~/tmp/speca` exists, is outside the Conflux worktree, and is a NyxFoundation/speca checkout.
- The checkout documents the current `scripts/run_phase.py` phases and arguments; installed docs/help win over older examples.
- Python dependencies are ready, or setup can be run from the SPECA checkout.
- Required Claude/API/session/auth access is available for the official runner without asking the user questions or logging secrets.

If any prerequisite is missing, unavailable, unauthenticated, or unsafe, record the limitation in human-readable reasoning and continue with manual SPECA-style property review.

### Observable setup and runner execution on mini

SPECA setup and execution may be long-running or noisy. On mini, run those commands through `agent-exec run -- ...` so progress remains observable and context-efficient. Run from the external SPECA checkout, not from the Conflux worktree. Examples:

```bash
agent-exec run -- uv sync
agent-exec run -- uv run python3 scripts/run_phase.py ...
```

Replace `...` with the phase and arguments supported by the checked-out NyxFoundation/speca version. Prepare input/output paths under `~/tmp/speca-conflux-input/<change-id>/` and `~/tmp/speca-conflux-output/<change-id>/` unless the installed runner documents a safer equivalent outside the Conflux worktree.

### Evidence classification and fallback

- Runner completes and produces relevant outputs: cite the output location in reasoning and use outputs as supporting proof/falsification evidence. Map any concrete blocking property failure to the standard JSON `fail` verdict with `findings`.
- Runner prerequisites are missing or auth/session access is unavailable: record the limitation, then perform manual SPECA-style review from repository evidence.
- Runner crashes, times out, or produces unusable output: record the failed command and output/log location, then perform manual SPECA-style review.
- Runner output conflicts with repository evidence, OpenSpec requirements, task claims, or `openspec/CONSTITUTION.md`: repository/workspace evidence is authoritative for pass/fail/continue/gated routing.

Never treat official SPECA runner output as durable workflow-control state. Never treat runner unavailability, setup failure, missing auth, or inconclusive output as an automatic pass or as a SPECA-specific protocol error.

## SPECA-Style Review Loop

### 1. Load baseline acceptance context

- Read the target change proposal, tasks, and spec deltas.
- Read changed implementation paths and test evidence from the workspace.
- Read `openspec/CONSTITUTION.md` when present.
- Apply the standard acceptance checks from the command template as authoritative.

### 2. Derive checkable properties

Derive candidate properties from repository evidence, including:

- OpenSpec requirements and scenarios.
- Task completion claims and their planned verification ownership.
- Changed files, public entry points, and integration call sites.
- Constitution constraints, especially workspace-local workflow control and truthful completion.
- Parser, prompt, and command-template contracts touched by the change.

Prefer properties that can be falsified with concrete file paths, functions, tests, or command output. Keep each property tied to a repository artifact so any finding is actionable.

### 3. Attempt proof or falsification

For each high-value property:

- Use local tests, static inspection, targeted command output, and changed-file analysis.
- If an external SPECA runner is installed and usable, use it as supporting evidence for proof-attempt structure.
- If SPECA tooling is unavailable, perform the same structured property review manually from repository evidence.
- Never treat unavailable SPECA tooling as an automatic pass.
- Never rely on out-of-worktree durable logs, caches, or UI state as authoritative workflow-control evidence.

### 4. Classify property outcomes

- **Blocking**: Concrete property failure with repository evidence. Map to standard acceptance `fail` with a `findings` item naming the property, evidence path/function/line when available, and required autonomous fix.
- **Advisory**: Non-blocking risk or improvement. Mention in reasoning if useful, but do not force failure by itself.
- **Incomplete**: Repository-only work/checks are still needed. Treat as `fail` when the agent can resolve it by editing code/tests/spec/tasks/docs.
- **Gated**: Use only when the standard acceptance blocker rubric allows it and repository-only work cannot resolve the issue.

### 5. Emit one Conflux verdict

After the SPECA-style pass, emit the final verdict using only the standard Conflux acceptance contract owned by `.opencode/commands/cflx-accept.md`. Missing or inconclusive proof attempts do not create a new protocol; they become standard findings only when they reveal an actionable acceptance failure.

## Autonomy and Workspace Rules

- Do not ask the user questions.
- Do not defer repository-fixable issues to humans.
- Base workflow-control decisions on workspace files, workspace git state, and base-branch comparison.
- Do not use out-of-worktree durable state to decide pass/fail/continue/gated.
- Do not change `acceptance_command` merely because this skill is selected; selection only changes the loaded operation skill.

## Built-in Tools

```bash
# Show change details
cflx openspec show <id>

# Validate change
cflx openspec validate <id> --strict
```
