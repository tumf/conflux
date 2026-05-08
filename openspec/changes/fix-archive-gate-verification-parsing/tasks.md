## Implementation Tasks

- [ ] Task 1: Replace brittle inline verification extraction in `src/openspec_cmd.rs` with parsing that captures the complete intended verification span while tolerating parenthesized/backticked command text inside the note. (verification: unit - add Rust tests in `src/openspec_cmd.rs` covering inline manual notes with source paths, backticked commands, and parenthesized command/prose segments; completion: evidence-bearing notes are parsed completely before evidence and ownership checks run.)
- [ ] Task 2: Preserve existing accepted verification syntaxes and existing strict failures. (verification: unit - extend `src/openspec_cmd.rs` tests for inline verification before completion prose, standalone indented verification continuation lines, missing verification notes, weak manual notes, missing ownership markers, and self-referential final validation checkboxes; completion: accepted cases still pass and invalid cases still produce the same class of findings.)
- [ ] Task 3: Add a regression fixture for the observed archive-gate false negative shape. (verification: unit - add a test in `src/openspec_cmd.rs` using wording equivalent to `Task 9` from `add-s3-workspace-persistence`, including source paths and `cflx openspec validate add-s3-workspace-persistence --strict`; completion: `validate_tasks_content` returns no evidence/ownership errors for that task.)
- [ ] Task 4: Run targeted validator tests. (verification: unit - run `cargo test openspec_cmd --lib` or the narrowest Rust test target covering `validate_tasks_content`; completion: the targeted tests pass without requiring heavy/default-excluded tests.)

## Future Work

- Consider adding a CLI-level fixture test that shells out to `cflx openspec validate <fixture> --archive-gate` for representative task files if a lightweight fixture harness is introduced later.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-archive-gate-verification-parsing --archive-gate`
