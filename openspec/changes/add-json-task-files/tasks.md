## Implementation Tasks

- [x] Introduce the shared task-file resolver with distinct progress, active-only, archived, and workspace-local mutation modes; add the format enum, versioned JSON model, semantic validation, progress parsing, JSON Pointer diagnostics, and atomic format-specific writer while preserving existing Markdown parsing and diagnostics. Add regressions for Markdown-only, JSON-only, selected-entry ambiguity, malformed and unsupported JSON, duplicate IDs, unknown sections/statuses, empty lists, extension-field preservation, and every location mode. (verification: unit - `cargo test --lib`; verification-id: json-task-file-tests)
- [x] Route native OpenSpec required-file checks, strict task validation, list/show progress, archive-layout recognition, manifests, diagnostics, and selected task-path reporting through the shared abstraction. Enforce active versus narrative classification, Final Validation non-task rules, verification linkage, and exact Git-diff add/delete basename pairing for JSON archives. (verification: integration - `cargo test --lib`; verification-id: json-task-file-tests)
- [x] Route Apply progress and format gates, Acceptance follow-up hydration/replacement/cleanup, rejection recovery, archive completion, sequential and parallel final-merge authorization, TUI/Web refresh, and resume detection through the shared abstraction. Prove internal findings remain virtual progress-gate tasks, external blockers retain current behavior, mutation never falls back to the base tree, legacy and structured findings round-trip, task-file errors fail closed, and refresh retains last-known progress. (verification: integration - `cargo test --lib`; verification-id: json-task-file-tests)
- [x] Update agent prompts, embedded cflx skills, and user documentation to refer to the resolved task file and give format-specific status-update rules. Keep `tasks.md` as the proposal default; document the JSON v1 active-task, narrative, follow-up, ownership, archive, diagnostics, and compatibility contracts. Add source assertions covering production prompts and embedded skills. (verification: integration - `cargo test --lib`; verification-id: json-task-file-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate add-json-task-files --archive-gate`.

## Current Acceptance Follow-up
- attempt: 1
- [x] Investigate acceptance failure and apply the required fix
  evidence: src/orchestration/acceptance.rs semantic_progress_fingerprint now recognizes both task basenames via crate::task_file::TaskFileFormat::from_file_name, so a JSON-only change's task artifact is included
  evidence: src/orchestration/acceptance.rs strip_runtime_follow_up keeps the Markdown section split and, for tasks.json, removes the acceptance_follow_up key before hashing so a runtime FAIL write is never Apply progress
  evidence: src/task_file.rs FOLLOW_UP_KEY is now pub so progress detection drops runtime bookkeeping without re-spelling the key
  evidence: src/orchestration/acceptance.rs semantic_fingerprint_tracks_json_tasks_but_excludes_runtime_follow_up asserts pending->completed changes the fingerprint and an acceptance_follow_up rewrite does not
  evidence: src/orchestration/acceptance.rs semantic_fingerprint_hashes_malformed_json_tasks_verbatim covers unparsable task bytes staying progress-visible
  evidence: skills/cflx-rejection-guide/SKILL.md now names the tasks_path artifact in both formats, records recovery work in that file's own format, and forbids creating the other filename
  evidence: skills/cflx-workflow/SKILL.md Apply/Rejecting/Acceptance sections route through tasks_path with Markdown checkbox and JSON status/narrative rules
  evidence: skills/cflx-accept-with-speca/SKILL.md mirrors cflx-accept read-only wording naming versioned tasks.json and the acceptance_follow_up object
  evidence: skills/README.md task-update guidance names the resolved artifact and gives per-format completion rules
  evidence: src/embedded_skills.rs test_embedded_skills_describe_both_task_file_formats now asserts the rejection-guide, workflow and accept-with-speca skills name the resolved artifact and forbid the second filename
  evidence: cargo fmt --all -- --check passed; cargo clippy --locked --all-targets --all-features -- -D warnings passed; cargo test --lib 4282 passed with only the 10 pre-existing tui::render header failures caused by the 66-column crate version
