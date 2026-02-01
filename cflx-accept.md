---
description: Run Conflux acceptance review (prompt provided by orchestrator)
---

You are running Conflux acceptance review.

The variable context is provided below (includes change_id and paths):

$ARGUMENTS

IMPORTANT:
- Only review the specific change referenced by the provided change_id/paths.
- Do NOT review or report on other changes in openspec/changes/.

Review the implementation to verify it meets the specification requirements.

External dependency policy (mock-first):
- Any requirement that AI cannot resolve or verify autonomously is an external dependency
- External dependencies that CAN be mocked/stubbed/fixtured MUST be mocked to enable verification without external credentials
- Only truly non-mockable external dependencies (requiring real external systems, human decisions, or long-wait verification) may be moved to Out of Scope / Future Work (without checkboxes)
- Missing secrets (API keys, credentials) MUST NOT be treated as a reason to output CONTINUE
- If verification requires secrets and no mock exists, output FAIL with specific follow-up tasks:
  * Implement mock/stub/fixture for the external dependency, OR
  * Move to Out of Scope as non-mockable (remove checkbox)

Required checks:
1. Git working tree clean check: run `git status --porcelain` and verify the output is empty.
   - If `git status --porcelain` produces any output (uncommitted changes or untracked files), it is a FAIL.
   - When FAIL due to dirty working tree, list ALL uncommitted changes and untracked files in FINDINGS with their file paths.
2. All tasks in openspec/changes/<change_id>/tasks.md are completed (marked with [x]) or moved to Future Work section
3. Checkbox removal check: If tasks are moved to "Future Work", "Out of Scope", or "Notes" sections, they MUST NOT have checkboxes (`- [ ]` or `- [x]`).
4. Implementation matches the specification in openspec/changes/<change_id>/specs/
5. Integration check: confirm the feature is actually executed in the real flow.
6. Dead code check: if code exists but is not invoked by the CLI/TUI/parallel flow described in spec, it is a FAIL.
7. Regression check: verify that existing features unrelated to this change are not broken.
8. Evidence: cite at least one file path + function/method where the integration happens.

FINDINGS format requirements:
- Each finding MUST include concrete evidence (file path, function name, line number if relevant)
- Each finding MUST be actionable by the AI agent autonomously

Output format (output exactly ONCE at the end):
- If all checks pass: Output "ACCEPTANCE: PASS"
- If checks fail: Output "ACCEPTANCE: FAIL" followed by FINDINGS and tasks.md update
- If verification cannot complete in this session: Output "ACCEPTANCE: CONTINUE"

CRITICAL - When outputting FAIL:
1. List ALL issues discovered in the FINDINGS section
2. After listing all findings, update openspec/changes/<change_id>/tasks.md:
   - Determine the next acceptance attempt number (check existing "## Acceptance #N Failure Follow-up" sections)
   - Append or create the section for that attempt
   - Add each finding as a separate unchecked task: "- [ ] <finding>"
