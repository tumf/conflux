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
- Refer to `openspec/AGENTS.md` (located inside the `openspec/` directory—run `ls openspec` or `npx @fission-ai/openspec@latest update` if you don't see it) if you need additional OpenSpec conventions or clarifications.

**Steps**
1. Determine the change ID to archive:
   - If this prompt already includes a specific change ID (for example inside a `<ChangeId>` block populated by slash-command arguments), use that value after trimming whitespace.
   - If `$ARGUMENTS` does not contain a change ID but the conversation context clearly indicates which change to archive, infer the change ID from context without asking.
   - If the conversation references a change loosely (for example by title or summary), run `npx @fission-ai/openspec@latest list` to surface likely IDs and pick the single best match without asking.
   - Otherwise, review the conversation, run `npx @fission-ai/openspec@latest list`, and ask the user which change to archive; wait for a confirmed change ID before proceeding.
   - If you still cannot identify a single change ID, stop and tell the user you cannot archive anything yet.
2. Validate the change ID by running `npx @fission-ai/openspec@latest list` (or `npx @fission-ai/openspec@latest show <id>`) and stop if the change is missing, already archived, or otherwise not ready to archive.
3. Run `npx @fission-ai/openspec@latest archive <id> --yes` so the CLI moves the change and applies spec updates without prompts (use `--skip-specs` only for tooling-only work).
4. Review the command output to confirm the target specs were updated and the change landed in `changes/archive/`.
5. Validate with `npx @fission-ai/openspec@latest validate --strict` and inspect with `npx @fission-ai/openspec@latest show <id>` if anything looks off.

**Reference**
- Use `npx @fission-ai/openspec@latest list` to confirm change IDs before archiving.
- Inspect refreshed specs with `npx @fission-ai/openspec@latest list --specs` and address any validation issues before handing off.
<!-- OPENSPEC:END -->
