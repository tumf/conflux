---
name: cflx-accept-with-speca
description: Portable Conflux acceptance review with an additional SPECA-style property/proof-attempt lens. Drop-in compatible with cflx-accept and cannot ask questions or request user input.
---

# Conflux Acceptance Review with SPECA Lens

Use this skill when the acceptance operation should add SPECA-style property review to the standard Conflux acceptance process.

**CRITICAL**: This skill CANNOT ask questions to users. Make autonomous acceptance judgments from repository evidence only.

## Purpose

This skill is a drop-in replacement for `cflx-accept` as a Conflux acceptance operation skill. It adds a property-oriented falsification pass to Conflux acceptance without changing the portable verdict output interface, accepted outcomes, retry semantics, or workflow-control meaning.

This skill is agent-runtime independent and may be loaded by any supported agent runtime. Runtime-specific entrypoints are adapters that may mirror the Conflux acceptance contract, but they are not required for this skill to produce a correct verdict.

If `openspec/CONSTITUTION.md` exists, read it before acceptance review and treat it as higher-priority project law than proposal/spec deltas when judging correctness.

## Operation Identity

- **Mode**: Acceptance review
- **Additional lens**: SPECA-style property derivation and proof/falsification attempt
- **Goal**: Verify that implementation evidence satisfies OpenSpec requirements, task claims, and derived properties
- **Output**: Use the exact same verdict output interface as `cflx-accept`

## Portable Verdict Interface Constraint

This skill MUST expose the exact same verdict output interface as `cflx-accept`. The SPECA lens may add reasoning and findings, but MUST NOT change the output schema, accepted outcomes, retry semantics, or final machine-readable verdict format.

Primary verdict:

- PASS: `{"acceptance":"pass"}`
- FAIL: `{"acceptance":"fail","findings":["<evidence>"]}`
- CONTINUE: `{"acceptance":"continue"}`
- Stalled hold compatibility handoff: `{"acceptance":"gated"}`

Backward-compatible fallback markers:

- `ACCEPTANCE: PASS`
- `ACCEPTANCE: FAIL`
- `ACCEPTANCE: CONTINUE`
- `ACCEPTANCE: GATED`
- Legacy fallback accepted during migration: `ACCEPTANCE: BLOCKED`

Use the standard Conflux acceptance outcomes only: `pass`, `fail`, `continue`, or the current stalled-hold compatibility token `gated`. For blocking SPECA/property failures that are repository-fixable, return the standard JSON `fail` verdict with actionable `findings` under the portable Conflux acceptance interface. Valid Implementation Blockers still create stalled acceptance holds and use the shared `{"acceptance":"gated"}` compatibility handoff until parser support for a `stalled` verdict exists. Do not emit any SPECA-specific terminal marker, alternate verdict line, alternate schema, or extra machine-readable verdict object. During JSON rollout, follow `cflx-accept` transition behavior by emitting JSON first and the matching legacy marker second as the final two lines when compatibility with older runtimes is required.

## Verification Completion Ownership

Apply the same completion-ownership rule as `cflx-accept`: the parent acceptance agent retains ownership of every verification it starts and MUST wait for the final result of every command, sub-agent, job, or monitored verification before emitting the final verdict. This includes SPECA runner phases and any other asynchronous or long-running verification work started during acceptance.

- If a verification result arrives asynchronously (for example through a completion notification or a background job), keep waiting until the final result is received and evaluated. Do not exit while owned verification work is still running.
- Progress prose, a waiting/status message, or a promise to decide after a future completion notification is not a valid terminal acceptance response. Only the canonical verdict terminates acceptance.
- This rule is portable and does not depend on a named runtime-specific monitoring tool. Whatever mechanism the current runtime uses to run or observe verification work, the parent agent must obtain the final result before terminating.

The Conflux runtime classifies a completed acceptance run that emits no canonical verdict as a missing-verdict protocol failure. It is not treated as an intentional `CONTINUE` and does not use the explicit-CONTINUE retry path.

## Declared Verification Phases

Apply the same structured verification-phase semantics as `cflx-accept`. `pre-integration` declarations require current-revision repository evidence and runnable local verification. `post-integration` declarations require review of repository-automation ownership, tracked repository automation, trigger, evidence publication, rerun action, prerequisites, and fixture/local evidence; do not fetch an undeployed or external target.

Missing, placeholder, or incorrectly wired repository automation is a repository-fixable FAIL. A correctly wired post-integration declaration with pending operational evidence is not a FAIL. A non-mockable external prerequisite that makes the declared automation unusable is a stalled hold with the prerequisite owner and next rerun or unblock action preserved. Never claim an unobserved operational outcome succeeded.

## Official NyxFoundation/speca Runner Adapter (Optional)

Attempt the official NyxFoundation/speca runner when it is locally available and usable. This runner is a supporting proof/falsification helper only; it is not required for acceptance and it never replaces repository evidence or the final Conflux verdict contract.

### Workspace boundary and artifact locations

Keep all official SPECA runner artifacts outside the target Conflux worktree by default, and namespace generated run artifacts by project/workspace so multiple Conflux projects and concurrent acceptance attempts do not collide:

- Operator-provisioned SPECA source checkout: `$SPECA_CHECKOUT` or `~/services/speca/`
- Per-attempt SPECA run worktree/copy: `~/tmp/cflx-speca/<workspace-key>/run/<change-id>/<attempt-id>/speca/`
- Generated Conflux/OpenSpec input bundle: `~/tmp/cflx-speca/<workspace-key>/input/<change-id>/<attempt-id>/`
- Official SPECA outputs and logs: `~/tmp/cflx-speca/<workspace-key>/output/<change-id>/<attempt-id>/` or a documented output directory inside the scoped external SPECA workspace

`<workspace-key>` MUST be stable for the target project/workspace and collision-resistant across projects, for example `<repo-basename>-<hash12>` derived from the canonical repository root path, git common directory, or git remote identity. `<attempt-id>` MUST distinguish repeated or concurrent acceptance attempts for the same change, for example a UTC timestamp plus process id or random suffix. Do not put the SPECA source checkout under `<change-id>` or `<attempt-id>`; use an operator-provisioned checkout and create isolated per-attempt run worktrees/copies from it. Do not use global input/output paths keyed only by `<change-id>`, because different projects may use the same OpenSpec change id.

Do not clone or install NyxFoundation/speca from this acceptance skill. Do not write generated inputs, store runner outputs, or place runner logs inside tracked Conflux paths unless a separate implementation task explicitly asks for a tracked fixture. Deleting the out-of-worktree SPECA input/output/log/cache directories must not change the next Conflux action for the same workspace file state and git state.

### Prerequisite checks before running

Before launching setup or execution, derive or select the scoped external SPECA workspace and verify:

- `uv` is installed and available on `PATH`.
- `$SPECA_CHECKOUT` is set to an existing NyxFoundation/speca checkout, or the default operator-managed checkout `~/services/speca/` exists.
- The selected checkout is outside the Conflux worktree and documents the current `scripts/run_phase.py` phases and arguments; installed docs/help win over older examples.
- Python dependencies are ready in that checkout, or `uv sync` can be run there without installing SPECA itself.
- Required model/API/session/auth access is available for the official runner without asking the user questions or logging secrets.

If any prerequisite is missing, unavailable, unauthenticated, or unsafe, record the limitation in human-readable reasoning and continue with manual SPECA-style property review.

### Observable setup and runner execution

SPECA setup and execution may be long-running or noisy. Use the current runtime's standard long-running-command mechanism when available so progress remains observable and context-efficient. Run from the scoped external SPECA checkout, not from the Conflux worktree.

Use these concrete commands as the default adapter shape. Replace `<workspace-key>`, `<change-id>`, `<attempt-id>`, `<spec-url>`, `<target-repo-url>`, `<target-commit>`, and `<target-language>` before running.

#### 1. Select the operator-provisioned SPECA checkout

```bash
SPECA_CHECKOUT="${SPECA_CHECKOUT:-$HOME/services/speca}"
if [ ! -d "$SPECA_CHECKOUT/.git" ]; then
  printf 'SPECA checkout not found: %s\n' "$SPECA_CHECKOUT" >&2
  exit 1
fi
cd "$SPECA_CHECKOUT"
git status --short
uv sync
```

This acceptance skill does not clone, install, or globally configure SPECA. If `$SPECA_CHECKOUT` or `~/services/speca/` is missing, skip the official runner and continue with manual SPECA-style review. Installation, updates, Claude Code CLI setup, MCP registration, and authentication are operator-owned outside this skill.

#### 2. Inspect the installed runner interface

```bash
SPECA_CHECKOUT="${SPECA_CHECKOUT:-$HOME/services/speca}"
cd "$SPECA_CHECKOUT"
uv run python3 scripts/run_phase.py --help
```

Installed help and checked-out scripts are authoritative. Prefer the documented `--phase <ID>` and `--target <ID>` arguments shown by this command over stale examples.

#### 3. Prepare an isolated per-attempt run directory

```bash
mkdir -p "$HOME/tmp/cflx-speca/<workspace-key>/run/<change-id>/<attempt-id>"
mkdir -p "$HOME/tmp/cflx-speca/<workspace-key>/input/<change-id>/<attempt-id>"
mkdir -p "$HOME/tmp/cflx-speca/<workspace-key>/output/<change-id>/<attempt-id>"
SPECA_CHECKOUT="${SPECA_CHECKOUT:-$HOME/services/speca}"
rsync -a --delete --exclude .git "$SPECA_CHECKOUT/" "$HOME/tmp/cflx-speca/<workspace-key>/run/<change-id>/<attempt-id>/speca/"
cd "$HOME/tmp/cflx-speca/<workspace-key>/run/<change-id>/<attempt-id>/speca"
uv sync
```

SPECA expects its phase files under an `outputs/` directory inside the active SPECA run directory. For a Conflux acceptance attempt, use the per-attempt SPECA run copy above, then write only generated SPECA inputs there. Do not write these files inside the Conflux worktree or mutate the shared SPECA checkout with change-specific outputs.

Minimum `outputs/TARGET_INFO.json` shape:

```json
{
  "name": "<target-name>",
  "repo": "<target-repo-url>",
  "commit": "<target-commit>",
  "language": "<target-language>"
}
```

Minimum `outputs/BUG_BOUNTY_SCOPE.json` shape:

```json
{
  "in_scope": { "components": ["<component>"], "scope_restriction": "<scope>" },
  "out_of_scope": [],
  "conditional_scope": [],
  "trust_assumptions": {
    "workspace_change": { "trust_level": "UNTRUSTED", "rationale": "Acceptance must verify repository-visible changes against OpenSpec requirements." }
  },
  "severity_classification": {
    "CRITICAL": "Breaks mandatory workflow-control or safety invariants.",
    "HIGH": "Causes incorrect pass/fail/archive routing or accepts invalid implementation evidence.",
    "MEDIUM": "Materially weakens verification or produces misleading acceptance evidence.",
    "LOW": "Minor review-quality issue with limited workflow impact."
  },
  "deployment_context": { "type": "single-repository", "target_share": { "value": 1.0, "metric": "repository" } }
}
```

#### 4. Run a smoke-test discovery phase when spec URLs are available

```bash
cd "$HOME/tmp/cflx-speca/<workspace-key>/run/<change-id>/<attempt-id>/speca"
SPEC_URLS="<spec-url>" uv run python3 scripts/run_phase.py --phase 01a
```

For OpenSpec-only Conflux changes that do not have external specification URLs, do not invent URLs. Instead, stage the relevant OpenSpec proposal/spec text as local generated inputs and continue only if the installed SPECA runner supports that input shape; otherwise use manual property review.

#### 5. Run the documented end-to-end phase target when required inputs exist

```bash
cd "$HOME/tmp/cflx-speca/<workspace-key>/run/<change-id>/<attempt-id>/speca"
uv run python3 scripts/run_phase.py --target 04 --workers 4 --max-concurrent 64
```

The README documents outputs as `outputs/<phase_id>_PARTIAL_*.json`. After completion, copy or move relevant SPECA outputs/logs to:

```bash
mkdir -p "$HOME/tmp/cflx-speca/<workspace-key>/output/<change-id>/<attempt-id>"
```

#### 6. Run individual phases when resuming or narrowing evidence

```bash
cd "$HOME/tmp/cflx-speca/<workspace-key>/run/<change-id>/<attempt-id>/speca"
uv run python3 scripts/run_phase.py --phase 01a
uv run python3 scripts/run_phase.py --phase 01b
uv run python3 scripts/run_phase.py --phase 01e
uv run python3 scripts/run_phase.py --phase 02c
uv run python3 scripts/run_phase.py --phase 03
uv run python3 scripts/run_phase.py --phase 04
```

Use only the phases supported by the checked-out runner. Phase meanings from the README are: `01a` discovery, `01b` subgraph extraction, `01e` property generation, `02c` code location pre-resolution, `03` property-grounded audit, and `04` audit review.

#### 7. Validate the SPECA checkout when diagnosing runner failures

```bash
SPECA_CHECKOUT="${SPECA_CHECKOUT:-$HOME/services/speca}"
cd "$SPECA_CHECKOUT"
uv run python3 -m pytest tests/ -v --tb=short
```

Runner test failure is evidence about runner health only. It is not, by itself, a Conflux acceptance failure unless it reveals a repository-fixable issue in the target Conflux change.

Prepare input/output paths under `~/tmp/cflx-speca/<workspace-key>/input/<change-id>/<attempt-id>/` and `~/tmp/cflx-speca/<workspace-key>/output/<change-id>/<attempt-id>/` unless the installed runner documents a safer equivalent outside the Conflux worktree and inside the scoped external SPECA workspace. Do not clone, install, update, or globally configure SPECA from this acceptance operation.

### Evidence classification and fallback

- Runner completes and produces relevant outputs: cite the output location in reasoning and use outputs as supporting proof/falsification evidence. Map any concrete blocking property failure to the standard JSON `fail` verdict with `findings`.
- Runner prerequisites are missing or auth/session access is unavailable: record the limitation, then perform manual SPECA-style review from repository evidence.
- Runner crashes, times out, or produces unusable output: record the failed command and output/log location, then perform manual SPECA-style review.
- Runner output conflicts with repository evidence, OpenSpec requirements, task claims, or `openspec/CONSTITUTION.md`: repository/workspace evidence is authoritative for pass/fail/continue/stalled-hold routing; `gated` remains only the current compatibility token for that stalled hold.

Never treat official SPECA runner output as durable workflow-control state. Never treat runner unavailability, setup failure, missing auth, or inconclusive output as an automatic pass or as a SPECA-specific protocol error.

## SPECA-Style Review Loop

### 1. Load baseline acceptance context

- Read the target change proposal, tasks, and spec deltas.
- Read changed implementation paths and test evidence from the workspace.
- Read `openspec/CONSTITUTION.md` when present.
- Apply the standard `cflx-accept` acceptance checks as authoritative.
- Treat acceptance as read-only review: do not edit `tasks.md` or the runtime-owned `## Current Acceptance Follow-up` section, and do not convert findings into checkbox tasks. Return repository findings and external blockers with concrete evidence and next actions; runtime classifies and persists them.
- Final OpenSpec validation, archive-gate validation, and archive readiness are not implementation tasks; if they need documentation, require a non-checkbox `## Final Validation` or notes section.

### 2. Derive checkable properties

Derive candidate properties from repository evidence, including:

- OpenSpec requirements and scenarios.
- Task completion claims and their planned verification ownership.
- Changed files, public entry points, and integration call sites.
- Constitution constraints, especially workspace-local workflow control and truthful completion.
- Parser, prompt, skill, and runtime verdict contracts touched by the change.

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
- **Stalled hold**: Use only when the standard acceptance blocker rubric allows it and repository-only work cannot resolve the issue. During the compatibility period, emit the shared `{"acceptance":"gated"}` verdict for this hold; do not introduce `{"acceptance":"stalled"}` or any SPECA-specific outcome.

### 5. Emit one Conflux verdict

After the SPECA-style pass, emit the final verdict using only the portable Conflux acceptance contract shared with `cflx-accept`. Missing or inconclusive proof attempts do not create a new protocol; they become standard findings only when they reveal an actionable acceptance failure.

## Autonomy and Workspace Rules

- Do not ask the user questions.
- Do not defer repository-fixable issues to humans.
- Base workflow-control decisions on workspace files, workspace git state, and base-branch comparison.
- Do not use out-of-worktree durable state to decide pass/fail/continue/stalled-hold routing; `gated` is only the current parser-compatible token for stalled holds.
- Do not change `acceptance_command` merely because this skill is selected; selection only changes the loaded operation skill.

## Built-in Tools

```bash
# Show change details
cflx openspec show <id>

# Validate change
cflx openspec validate <id> --strict
```
