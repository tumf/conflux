## Implementation Tasks

- [x] Task 1: Add operation skill config fields and defaults. (verification: unit - add or update config tests in `src/config/types.rs`, `src/config/load.rs`, or adjacent config modules covering default values for `analyze_skill`, `apply_skill`, `rejecting_skill`, `cleanup_review_skill`, `accept_skill`, `archive_skill`, and `resolve_skill`; completion condition: omitted config preserves current `cflx-*` skill names)
- [x] Task 2: Implement config merge and validation for operation skill fields. (verification: unit - add merge precedence tests in `src/config/load.rs` or `src/config/types.rs` for at least `accept_skill` and one non-acceptance key such as `resolve_skill`, including custom/project/global override behavior and empty/newline value handling if validation is added; completion condition: higher-precedence configs override lower-precedence skill names consistently)
- [x] Task 3: Refactor prompt builders to use selected skill names. (verification: unit - update tests in `src/agent/prompt.rs`, `src/orchestration/selection.rs`, `src/orchestration/rejection.rs`, and `src/parallel/conflict.rs` to assert both default preludes and custom skill preludes; completion condition: hardcoded `load skills: cflx-*` is no longer the only possible output for orchestrator operation prompts)
- [x] Task 4: Thread configured skill names through runtime call sites. (verification: integration - inspect and test call paths from config-aware execution in `src/execution/apply.rs`, `src/agent/runner.rs`, `src/orchestration/selection.rs`, `src/orchestration/rejection.rs`, and `src/parallel/conflict.rs`; completion condition: analyze/apply/rejecting/cleanup-review/accept/archive/resolve all use effective config values)
- [x] Task 5: Preserve parser and workflow behavior. (verification: integration - run targeted tests such as `cargo test acceptance`, resolve/conflict prompt tests, and rejection review parser tests; completion condition: verdict parsing, conflict resolution markers, and rejection review markers are unchanged)
- [x] Task 6: Document operation skill configuration. (verification: manual - update `src/templates.rs` and any relevant docs/templates, then inspect those files for all new keys and at least one example using `"accept_skill": "cflx-accept-with-speca"`; completion condition: users can discover defaults and custom-skill usage from generated config/docs)
- [x] Task 7: Run formatting and targeted verification. (verification: manual - run `cargo fmt --check`, `cargo test config`, `cargo test prompt`, `cargo test embedded_skills` if touched, and targeted parser tests such as `cargo test acceptance`; completion condition: commands pass or any long-running checks are classified according to repository test policy)

## Future Work

- Concrete `cflx-accept-with-speca` skill implementation is handled by `add-speca-acceptance-skill`.
- A nested `operation_skills` map can be considered later if the flat key list becomes too large.
- Runtime skill availability probing can be added later if agent runtimes expose a stable skill inventory API.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate add-configurable-operation-skills --archive-gate`
