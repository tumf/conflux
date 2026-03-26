## 1. Post-run review commands
- [x] 1.1 Update `skills/cflx-run/SKILL.md` so the post-run command set includes direct inspection of canonical spec diffs under `openspec/specs/**` (verification: `skills/cflx-run/SKILL.md` documents a spec-diff command alongside `git status`, `git log`, and commit diff review)
- [x] 1.2 Document how operators should identify which archived changes landed before summarizing canonical spec updates (verification: `skills/cflx-run/SKILL.md` includes an explicit per-change review step)

## 2. Review checklist expectations
- [x] 2.1 Require the post-run summary to name the canonical specs changed by each archived change that landed (verification: `skills/cflx-run/SKILL.md` includes a per-change canonical-spec summary requirement)
- [x] 2.2 Require the post-run checklist to flag spec-only changes that land without a canonical spec diff (verification: `skills/cflx-run/SKILL.md` names the empty-diff anomaly and its expected reporting behavior)

## 3. Worked guidance
- [x] 3.1 Add an example or guidance snippet showing how commit review and canonical spec review complement each other in `cflx-run` (verification: `skills/cflx-run/SKILL.md` contains a worked explanation of both review layers)
