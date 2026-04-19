---
agent: build
description: Run Conflux acceptance review (prompt provided by orchestrator)
---

You are running Conflux acceptance review.

The variable context is provided below (includes change_id and paths):

$ARGUMENTS

IMPORTANT:
- Only review the specific change referenced by the provided change_id/paths.
- Do NOT review or report on other changes in openspec/changes/.

Review the implementation to verify it meets the specification requirements.

External dependency policy (production-first; mocks/stubs are test-only):
- Any requirement that AI cannot resolve or verify autonomously is an external dependency
- Production code MUST NOT rely on mocks/stubs/fakes as the default runtime implementation.
- Mocks/stubs/fakes are permitted ONLY in unit tests (and other test-only code paths such as `#[cfg(test)]` or `tests/`).
- External dependencies SHOULD be verified via unit tests that mock the external system, but the production implementation MUST still exist and be wired into the real execution flow.
- Missing secrets (API keys, credentials) MUST NOT be treated as a reason to output CONTINUE.
- If verification would require secrets:
  * Output FAIL with follow-up tasks that keep the real implementation and move mocking to tests (e.g., add a client interface, add unit tests with mocks/fixtures, and document configuration).
  * If the feature truly requires a live external system at runtime, output FAIL with follow-up tasks to add clear fail-fast behavior + actionable configuration docs (do NOT ship a stub as the implementation).

Permission Error Acceptance:
- If tasks.md contains a "## Future Work" section with permission-related tasks:
  - Verify the permission error explanation is clear and actionable
  - Verify the required permission configuration guidance is documented (for example `.cflx.jsonc`)
  - Verify all OTHER tasks (not requiring the blocked resource) are completed
  - If above conditions are met: Output "ACCEPTANCE: PASS"
  - If unblocked tasks remain incomplete: Output "ACCEPTANCE: FAIL" with findings
- If all unchecked tasks are permission-blocked and no actionable task was completed, output "ACCEPTANCE: FAIL" with findings (insufficient progress)

- Permission errors are NOT treated as Implementation Blockers (do NOT output "ACCEPTANCE: BLOCKED")
- Permission errors are expected workflow outcomes when file access is restricted

Implementation Blocker review (acceptance stage scope):
1. Check if tasks.md contains any "## Implementation Blocker #N" sections
2. Check whether `openspec/changes/<change_id>/REJECTED.md` exists (apply-generated rejection proposal artifact)
3. If both blocker section and `REJECTED.md` exist:
   - Treat this as a rejecting-stage handoff signal that should be reviewed by rejecting flow, not acceptance.
   - Output `ACCEPTANCE: FAIL` with finding that routing is incorrect if acceptance is invoked directly for this state.
4. If blocker section exists without `REJECTED.md`:
   - Treat as acceptance FAIL
   - Add finding requiring apply/rejecting flow to produce or clear blocker evidence consistently.
5. If blocker section does not exist, run normal acceptance checks below.

Spec-only change detection:
- Read `openspec/changes/<change_id>/proposal.md` and look for a `Change Type` field.
- If `Change Type: spec-only` is found, apply the **Spec-Only Acceptance Checks** below instead of the runtime integration / dead-code / stubbed-runtime checks.

Spec-Only Acceptance Checks (replace checks 4–7 for spec-only changes):
A. Archive-readiness simulation:
   - For each spec delta under `openspec/changes/<change_id>/specs/<capability>/spec.md`, check whether the delta contains at least one `## ADDED Requirements` section OR contains non-trivial `## MODIFIED Requirements` content that would change the canonical spec.
   - FAIL with finding "Archive no-op risk: promoting delta for <capability> would not change the canonical spec" when:
     a. A delta contains only `## MODIFIED Requirements` or `## REMOVED Requirements` sections, AND the corresponding target requirement cannot be located in `openspec/specs/<capability>/spec.md` (missing target = no-op promotion), OR
     b. A delta is empty or structurally invalid.
   - FAIL when archive simulation indicates a no-op promotion (i.e., the delta would produce no net change to the canonical spec).
B. Spec tasks completion: All `## Specification Tasks` entries must be `[x]` or in Future Work. Absence of runtime code is expected and NOT a failure.
C. No unrelated runtime evidence required: do NOT fail because source code, tests, or CLI wiring are absent. The change type is spec-only by design.

Required checks (only run if no valid Implementation Blocker exists):
1. Git working tree clean check: run `git status --porcelain` and verify the output is empty.
   - If `git status --porcelain` produces any output (uncommitted changes or untracked files), it is a FAIL.
   - When FAIL due to dirty working tree, list ALL uncommitted changes and untracked files in FINDINGS with their file paths.
2. All tasks in openspec/changes/<change_id>/tasks.md are completed (marked with [x]) or moved to Future Work section
3. Checkbox removal check: If tasks are moved to "Future Work", "Out of Scope", or "Notes" sections, they MUST NOT have checkboxes (`- [ ]` or `- [x]`).
4. Implementation matches the specification in openspec/changes/<change_id>/specs/
   - For `spec-only` changes: apply Spec-Only Acceptance Checks A–C above instead.
5. Integration check: confirm the feature is actually executed in the real flow.
   - Skip for `spec-only` changes (no runtime flow expected).
6. Dead code check: if code exists but is not invoked by the CLI/TUI/parallel flow described in spec, it is a FAIL.
   - Skip for `spec-only` changes (no runtime code expected).
7. No stubbed runtime check: FAIL if the real execution path uses a mock/stub/fake/placeholder implementation.
    - Examples of disallowed runtime placeholders: `todo!()`, `unimplemented!()`, always-empty returns, `Fake*`/`Mock*`/`Stub*` clients in non-test code, or feature-flagged mocks enabled by default.
    - Mocks/stubs/fakes are allowed only in test-only code paths (`#[cfg(test)]`, `tests/`).
    - Skip for `spec-only` changes.
8. Regression check: verify that existing features unrelated to this change are not broken.
9. Evidence: cite at least one file path + function/method where the integration happens.
   - For `spec-only` changes: cite the spec delta file path and the expected canonical promotion target.
10. Checklist truthfulness check:
   - FAIL if `tasks.md` marks implementation work complete but the repository contains only `openspec/` edits for that claimed work.
   - FAIL if a task marked `[x]` claims runtime behavior, code, tests, or wiring that cannot be located in the repo.
   - FAIL if a merge/archive/spec-promotion occurred without corresponding implementation evidence for completed tasks.
   - FAIL if acceptance evidence relies only on proposal/spec/task documents rather than executable code/tests/integration points.
   - FAIL on unit/integration classification mismatch: when a task claims unit-test coverage but the cited tests rely on real stateful external boundaries (e.g., real git repo, real CLI/process, real filesystem/database/network/timer).
   - For mismatch FAILs, require follow-up tasks to either (a) extract pure decision logic and add genuine unit-scoped tests with mocks/fakes, or (b) reclassify the tests/checklist claim as integration/e2e.
   - Exception for `spec-only` changes: `openspec/` edits ARE the implementation artifact; runtime evidence is NOT required.

When evaluating completed tasks, use this evidence hierarchy:
1. Real entrypoint or call-site wiring in non-test code
2. Tests that exercise the claimed behavior
3. Build/lint/typecheck output proving the relevant files participate in the repo
4. `openspec/` documents only as supporting context, never as sole proof of implementation

FINDINGS format requirements:
- Each finding MUST include concrete evidence (file path, function name, line number if relevant)
- Each finding MUST be actionable by the AI agent autonomously
- If the problem is false completion, the finding MUST explicitly name the task/checklist claim that is unsupported

Output format (output exactly ONCE at the end):

**Primary (REQUIRED)** — emit a strict JSON verdict object as the LAST
machine-readable payload, as its own line:

- PASS:     `{"acceptance":"pass"}`
- FAIL:     `{"acceptance":"fail","findings":["<evidence 1>","<evidence 2>"]}`
- CONTINUE: `{"acceptance":"continue"}`
- BLOCKED:  `{"acceptance":"blocked"}`

The JSON verdict is the canonical machine-readable contract. For FAIL the
`findings` array mirrors the FINDINGS section.

**Fallback (backward-compatible)** — older runs still recognize legacy
standalone plain-text markers on their own line. These remain supported so
existing runs do not break:

- `ACCEPTANCE: PASS`
- `ACCEPTANCE: FAIL`
- `ACCEPTANCE: CONTINUE`
- `ACCEPTANCE: BLOCKED`

When both a JSON verdict and a legacy text marker appear, the JSON verdict
wins. Prefer the JSON contract.

**Transition guidance — emit BOTH during rollout** — until all running
Conflux orchestrator processes have been rebuilt with the JSON-aware
acceptance parser, the agent MUST emit BOTH payloads as the final two lines
of stdout (JSON verdict first, legacy marker second), each on its own line
with no markdown wrapping. Newer runtimes resolve the JSON verdict and
finalize; older runtimes still finalize on the legacy marker. The canonical
contract remains JSON-primary.

CRITICAL formatting rule: The JSON verdict (or the legacy fallback marker)
MUST be on its own line with NOTHING else on that line. Do NOT wrap it in
markdown formatting.

The runtime parser enforces the contract strictly. Any line that is not a
valid strict JSON verdict object and does not equal one of the legacy
markers exactly (after stripping defensively-tolerated
bold/italic/heading/blockquote/bullet prefixes) is rejected and treated as
if no verdict was emitted, so acceptance will retry. The runtime also
finalizes acceptance the moment a JSON verdict or a canonical standalone
text marker is observed in stdout — including when wrapped inside an
`opencode run --format json` assistant/result event — which means emitting
the verdict immediately ends acceptance even if the agent process keeps
stdout open afterwards.

Forbidden wrappings (will cause parser failures or unintended fallback):
- NO markdown headings: "## ACCEPTANCE: PASS" is WRONG
- NO blockquotes: "> ACCEPTANCE: PASS" is WRONG
- NO bullets: "- ACCEPTANCE: PASS" is WRONG
- NO bold/italic: "**ACCEPTANCE: PASS**" is WRONG
- NO code fences: wrapping in ``` blocks is WRONG
- NO trailing text: "ACCEPTANCE: PASSAll criteria verified" is WRONG
- NO heading concatenation: "ACCEPTANCE: PASS## Acceptance Review Summary" is WRONG

Correct output: "ACCEPTANCE: PASS" alone on its own line, followed by a newline.

CRITICAL - When outputting FAIL:
1. List ALL issues discovered in the FINDINGS section
2. After listing all findings, update openspec/changes/<change_id>/tasks.md:
    - Determine the next acceptance attempt number (check existing "## Acceptance #N Failure Follow-up" sections)
    - Append or create the section for that attempt
    - Add each finding as a separate unchecked task: "- [ ] <finding>"

Examples of mandatory FAIL cases:
- `tasks.md` says loop worker/CLI/runtime flow is complete, but no worker/CLI/runtime files exist outside `openspec/`
- tasks are all `[x]`, but the only diff in the implementation commit is proposal/spec/tasks edits
- spec requirements were promoted to canonical specs while source/test integration is still absent
