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

Implementation Blocker review:
1. Check if tasks.md contains any "## Implementation Blocker #N" sections
2. If Implementation Blocker(s) exist:
   a. Review each blocker's Category, Root Cause, Evidence, Impact, and Resolution Required
   b. Verify the blocker is legitimate (spec contradiction or truly non-mockable external constraint)
   c. If blocker is valid:
      - Output "ACCEPTANCE: BLOCKED"
      - Do NOT output FINDINGS or update tasks.md
      - The orchestrator will stop the apply loop and preserve the workspace
   d. If blocker is NOT valid (issue is mockable or solvable autonomously):
      - Treat as acceptance FAIL
      - Add finding: "Implementation Blocker #N is not valid: [reason]. Agent must [specific action]."

Required checks (only run if no valid Implementation Blocker exists):
1. Git working tree clean check: run `git status --porcelain` and verify the output is empty.
   - If `git status --porcelain` produces any output (uncommitted changes or untracked files), it is a FAIL.
   - When FAIL due to dirty working tree, list ALL uncommitted changes and untracked files in FINDINGS with their file paths.
2. All tasks in openspec/changes/<change_id>/tasks.md are completed (marked with [x]) or moved to Future Work section
3. Checkbox removal check: If tasks are moved to "Future Work", "Out of Scope", or "Notes" sections, they MUST NOT have checkboxes (`- [ ]` or `- [x]`).
4. Implementation matches the specification in openspec/changes/<change_id>/specs/
5. Integration check: confirm the feature is actually executed in the real flow.
6. Dead code check: if code exists but is not invoked by the CLI/TUI/parallel flow described in spec, it is a FAIL.
7. No stubbed runtime check: FAIL if the real execution path uses a mock/stub/fake/placeholder implementation.
   - Examples of disallowed runtime placeholders: `todo!()`, `unimplemented!()`, always-empty returns, `Fake*`/`Mock*`/`Stub*` clients in non-test code, or feature-flagged mocks enabled by default.
   - Mocks/stubs/fakes are allowed only in test-only code paths (`#[cfg(test)]`, `tests/`).
8. Regression check: verify that existing features unrelated to this change are not broken.
9. Evidence: cite at least one file path + function/method where the integration happens.

FINDINGS format requirements:
- Each finding MUST include concrete evidence (file path, function name, line number if relevant)
- Each finding MUST be actionable by the AI agent autonomously

Output format (output exactly ONCE at the end):

**Primary (REQUIRED)** — emit a strict JSON verdict object as the LAST
machine-readable payload, as its own line, with NOTHING else on that line:

- PASS:     `{"acceptance":"pass"}`
- FAIL:     `{"acceptance":"fail","findings":["<evidence 1>","<evidence 2>"]}`
- CONTINUE: `{"acceptance":"continue"}`
- BLOCKED:  `{"acceptance":"blocked"}`

Rules for the JSON verdict:

- It MUST be a valid JSON object on a single line (no pretty-printing).
- The `acceptance` field value MUST be one of `pass`, `fail`, `continue`,
  `blocked` (case-insensitive, but lowercase is preferred).
- For FAIL, `findings` MUST be an array of strings where each entry is a
  concrete, actionable finding (file path + function/line when applicable).
- Do NOT wrap the JSON verdict in code fences, markdown, or prose.
- The runtime finalizes acceptance the moment the JSON verdict line is
  observed in stdout — including when emitted inside an `opencode run
  --format json` assistant/result event — so emit it as the final payload.

**Fallback (backward-compatible)** — older runtimes still recognize the
legacy standalone plain-text markers, which remain supported so existing
runs do not break:

- `ACCEPTANCE: PASS`
- `ACCEPTANCE: FAIL`
- `ACCEPTANCE: CONTINUE`
- `ACCEPTANCE: BLOCKED`

These legacy markers are fallback only. When both a JSON verdict and a text
marker appear, the JSON verdict wins. Prefer the JSON contract.

CRITICAL formatting rule: The JSON verdict (or the legacy fallback marker)
MUST be on its own line with NOTHING else on that line. Do NOT wrap it in
markdown formatting.

This canonical verdict contract is owned by THIS command template. The
runtime parser enforces the contract strictly:

- Any line that is not a valid strict JSON verdict object and does not equal
  one of the legacy markers exactly (after stripping defensively-tolerated
  bold/italic/heading/blockquote/bullet prefixes) is rejected and treated as
  if no verdict was emitted, which forces acceptance to retry.

Forbidden wrappings (will cause parser failures or unintended fallback):
- NO markdown headings: "## ACCEPTANCE: PASS" is WRONG
- NO blockquotes: "> ACCEPTANCE: PASS" is WRONG
- NO bullets: "- ACCEPTANCE: PASS" is WRONG
- NO bold/italic: "**ACCEPTANCE: PASS**" is WRONG
- NO code fences: wrapping either the JSON verdict or the legacy marker in
  ``` blocks is WRONG
- NO trailing text: "ACCEPTANCE: PASSAll criteria verified" is WRONG
- NO heading concatenation: "ACCEPTANCE: PASS## Acceptance Review Summary"
  is WRONG
- NO multi-line JSON: the verdict JSON MUST fit on a single line

Correct primary output:
`{"acceptance":"pass"}` alone on its own line, followed by a newline.

Correct fallback output:
`ACCEPTANCE: PASS` alone on its own line, followed by a newline.

CRITICAL - When outputting FAIL:
1. List ALL issues discovered in the FINDINGS section (for human readers) AND
   include them in the `findings` array of the JSON verdict (for the runtime).
2. After listing all findings, update openspec/changes/<change_id>/tasks.md:
   - Determine the next acceptance attempt number (check existing "## Acceptance #N Failure Follow-up" sections)
   - Append or create the section for that attempt
   - Add each finding as a separate unchecked task: "- [ ] <finding>"
