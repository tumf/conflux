---
agent: build
description: Archive a deployed OpenSpec change and update specs.
---
change_id: $1

$ARGUMENTS

**MUST**: The files under `openspec/specs/**` must be written in English.

<!-- OPENSPEC:START -->
**Guardrails**
- Favor straightforward, minimal implementations first and add complexity only when it is requested or clearly required.
- Keep changes tightly scoped to the requested outcome.


**Steps**
1. Determine the change ID to archive:
   - If this prompt already includes a specific change ID (for example inside a `<ChangeId>` block populated by slash-command arguments), use that value after trimming whitespace.
   - If `$ARGUMENTS` does not contain a change ID but the conversation context clearly indicates which change to archive, infer the change ID from context without asking.
   - If the conversation references a change loosely (for example by title or summary), run `python3 "<SKILL_ROOT>/scripts/cflx.py" list` to surface likely IDs and pick the single best match without asking.
   - Otherwise, review the conversation, run `python3 "<SKILL_ROOT>/scripts/cflx.py" list`, and ask the user which change to archive; wait for a confirmed change ID before proceeding.
   - If you still cannot identify a single change ID, stop and tell the user you cannot archive anything yet.
2. Validate the change ID by running `python3 "<SKILL_ROOT>/scripts/cflx.py" list` (or `python3 "<SKILL_ROOT>/scripts/cflx.py" show <id>`) and stop if the change is missing, already archived, or otherwise not ready to archive.
3. Run `python3 "<SKILL_ROOT>/scripts/cflx.py" archive <id> --yes` so the CLI moves the change and applies spec updates without prompts (use `--skip-specs` only for tooling-only work).
4. Review the command output to confirm the target specs were updated and the change landed in `changes/archive/`.
5. **Verify the canonical spec diff**: run `git diff openspec/specs/` and confirm each touched `openspec/specs/**` file shows the expected requirement additions, replacements, or removals. Do not treat `Specs updated: [...]` output alone as sufficient evidence that specs changed correctly.
6. Validate with `python3 "<SKILL_ROOT>/scripts/cflx.py" validate --strict` and inspect with `python3 "<SKILL_ROOT>/scripts/cflx.py" show <id>` if anything looks off.

**Reference**
- Use `python3 "<SKILL_ROOT>/scripts/cflx.py" list` to confirm change IDs before archiving.
- Inspect refreshed specs with `python3 "<SKILL_ROOT>/scripts/cflx.py" list --specs` and address any validation issues before handing off.
- Always verify `git diff openspec/specs/` after archiving — the archive command rejects silent no-op promotions, but reviewing the diff confirms requirement blocks were added, replaced, or deleted as intended.
<!-- OPENSPEC:END -->
