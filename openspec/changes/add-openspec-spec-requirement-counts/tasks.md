# Tasks

## Implementation

1. [x] Extend native spec list metadata to include canonical requirement counts (verification: `src/openspec_cmd.rs` includes a spec listing field for requirement counts and `cargo test openspec_cmd` passes relevant unit tests)

2. [x] Count `### Requirement:` headings while scanning `openspec/specs/*/spec.md` (verification: add unit coverage in `src/openspec_cmd.rs` for nonzero and zero-count specs, then run `cargo test openspec_cmd`)

3. [x] Render `Requirements: <n>` in `cflx openspec list --specs` human-readable output without changing spec ordering or path lines (verification: add/update output-focused tests in `src/openspec_cmd.rs`, then run `cargo test openspec_cmd`)

4. [x] Update CLI spec coverage for canonical spec requirement counts (verification: `openspec/changes/add-openspec-spec-requirement-counts/specs/cli/spec.md` describes the new output behavior and `cflx openspec validate add-openspec-spec-requirement-counts --strict` passes)
