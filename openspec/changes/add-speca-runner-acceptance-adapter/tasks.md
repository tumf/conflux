## Implementation Tasks

- [ ] Task 1: Extend `skills/cflx-accept-with-speca/SKILL.md` with an official NyxFoundation/speca runner adapter workflow. (verification: unit - embedded skill contract test asserts the skill mentions NyxFoundation/speca, `~/tmp/speca`, and `uv run python3 scripts/run_phase.py`; completion condition: the embedded skill text tells acceptance reviewers when and how to attempt the official runner)
- [ ] Task 2: Add safe workspace-boundary guidance for SPECA runner artifacts. (verification: unit - `cargo test embedded_skills` with assertions in `src/embedded_skills.rs` that the skill directs SPECA clones, generated inputs, outputs, and logs outside the Conflux worktree and preserves workspace-local workflow-control authority; completion condition: the skill cannot be interpreted as writing runner artifacts into tracked Conflux paths by default)
- [ ] Task 3: Add prerequisite and failure-fallback guidance for runner execution. (verification: unit - `cargo test embedded_skills` with assertions in `src/embedded_skills.rs` that the skill checks `uv`, SPECA dependencies, and Claude/API/session availability, and falls back to manual SPECA-style review when execution is unavailable; completion condition: unavailable or failed runner execution is neither an automatic pass nor a protocol error)
- [ ] Task 4: Add observable long-command guidance for mini. (verification: unit - embedded skill contract test asserts runner setup/execution examples use `agent-exec run --` for `uv sync` or `uv run python3 scripts/run_phase.py`; completion condition: long/noisy runner work is documented as managed and observable)
- [ ] Task 5: Preserve acceptance verdict ownership and no alternate terminal protocol. (verification: unit - `cargo test embedded_skills` continues to pass existing drift tests for `.opencode/commands/cflx-accept.md` ownership and forbidden `SPECA: PASS/FAIL/CONTINUE/GATED` markers; completion condition: the updated skill still maps blocking property failures only to standard Conflux acceptance findings)

## Final Validation

Expected validation before acceptance: `cargo test embedded_skills` and `cflx openspec validate add-speca-runner-acceptance-adapter --strict --evidence warn`.
Expected archive gate: `cflx openspec validate add-speca-runner-acceptance-adapter --archive-gate`
