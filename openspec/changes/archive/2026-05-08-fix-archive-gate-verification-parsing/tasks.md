## Implementation Tasks

- [x] Task 1: Replace brittle inline verification extraction in `src/openspec_cmd.rs` with parsing that captures the complete intended verification span while tolerating parenthesized/backticked command text inside the note. (verification: unit - added Rust tests in `src/openspec_cmd.rs` covering inline manual notes with source paths, backticked commands, and parenthesized command/prose segments; completion: `cargo test validation_tests --lib` passed and evidence-bearing notes are parsed completely before evidence and ownership checks run.)
- [x] Task 2: Preserve existing accepted verification syntaxes and existing strict failures. (verification: unit - `src/openspec_cmd.rs` tests cover inline verification before completion prose, standalone indented verification continuation lines, missing verification notes, weak manual notes, missing ownership markers, and self-referential final validation checkboxes; completion: `cargo test validation_tests --lib` passed, accepted cases still pass, and invalid cases still produce the same class of findings.)
- [x] Task 3: Add a regression fixture for the observed archive-gate false negative shape. (verification: unit - added `test_accepts_observed_archive_gate_manual_note_shape` in `src/openspec_cmd.rs` using wording equivalent to `Task 9` from `add-s3-workspace-persistence`, including source paths and `cflx openspec validate add-s3-workspace-persistence --strict`; completion: `cargo test validation_tests::test_accepts_observed_archive_gate_manual_note_shape --lib` passed and `validate_tasks_content` returns no evidence/ownership errors for that task.)
- [x] Task 4: Run targeted validator tests. (verification: unit - ran `cargo test openspec_cmd --lib`; completion: targeted tests passed without requiring heavy/default-excluded tests.)

## Future Work

- Consider adding a CLI-level fixture test that shells out to `cflx openspec validate <fixture> --archive-gate` for representative task files if a lightweight fixture harness is introduced later.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-archive-gate-verification-parsing --archive-gate`
