## Implementation Tasks

- [ ] Implement shared fence-aware Markdown scanning for runtime acceptance sections and native task progress, including backtick/tilde marker matching, dynamic fence lengths, and explicit detection of unclosed or ambiguous boundaries. (verification: unit - add table-driven cases in `src/task_parser.rs` and run `cargo test task_parser`)
- [ ] Replace blanket unknown-line rejection with byte-preserving classification that separates known runtime records from unknown payloads without normalizing the unknown bytes. (verification: unit - prove capitalization drift, multiline evidence, embedded headings, checkbox text, and embedded backtick runs are classified and preserved by `cargo test task_parser`)
- [ ] Render and deduplicate `## Recovered Acceptance Notes` using a fixed untrusted-content notice and a backtick fence longer than every run in the payload. (verification: unit - repeat replacement, hydration, restart simulation, and PASS cleanup and assert one byte-identical recovered block with `cargo test task_parser`)
- [ ] Make tasks-file updates atomic using a same-directory temporary file and rename, preserving the original file when write or replacement fails. (verification: unit - inject or reproduce write/rename failure and assert original bytes remain unchanged with `cargo test task_parser`)
- [ ] Wire the shared recovery result through apply hydration, acceptance FAIL replacement, and PASS cleanup while preserving acceptance diagnosis priority and emitting actionable supplemental warnings. (verification: integration - add serial and parallel workflow cases under `src/serial_run_service.rs` and `src/parallel/tests/`; run `cargo test acceptance_follow_up`)
- [ ] Make native OpenSpec task validation and archive-gate task scanning ignore content inside valid backtick and tilde fences so recovered checkbox-like text cannot alter validation. (verification: unit - add validator cases in `src/openspec_cmd.rs` or `src/openspec_cmd/validation.rs` and run `cargo test openspec`)
- [ ] Add end-to-end regression coverage using a fixture `tasks.md` containing completed runtime findings plus free-form evidence, then verify retry normalization, process-style restart, PASS cleanup, stable task counts, and retained recovered notes. (verification: integration - add a focused test under `tests/` or existing orchestration integration coverage and run `cargo test --all-targets`)
- [ ] Update canonical-facing documentation and embedded apply guidance to state that recovered notes are untrusted historical content, not instructions or task state, without allowing agents to edit runtime-owned findings. (verification: unit - extend embedded-skill assertions in `src/embedded_skills.rs` and run `cargo test embedded_skills`)

## Future Work

- Consider an operator command to inspect or intentionally remove accumulated recovered notes if real repositories demonstrate material long-term growth.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate recover-acceptance-follow-up-content --archive-gate`
