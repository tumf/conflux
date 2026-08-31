---
change_type: implementation
priority: high
dependencies: []
references:
  - src/openspec_cmd/verify.rs
  - src/archive_layout.rs
  - src/openspec_cmd.rs
  - src/orchestration/acceptance/verification_evidence.rs
  - openspec/specs/proposal-metadata/spec.md
  - openspec/specs/cli/spec.md
verifications:
  - id: archived-change-verification-resolution
    requirement: Runtime-supervised verification resolves one change from its active proposal or its canonical archive entry without weakening archive identity checks
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: Focused Rust tests in src/archive_layout.rs and src/openspec_cmd/verify.rs cover active precedence, dated and direct archive entries, archive-only execution, invalid nested layouts, ambiguous archives, and missing changes
    rerun: cargo test --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Resolve archived changes in `cflx openspec verify`

**Change Type**: implementation

## Problem / Context

`cflx openspec verify <change-id>` resolves declarations only from `openspec/changes/<change-id>/proposal.md`. Native archive moves that proposal to `openspec/changes/archive/YYYY-MM-DD-<change-id>/proposal.md`, so the same logical verification ID becomes unreadable after successful archive even though the declaration and repository-level automation remain valid.

Conflux already owns canonical archive-entry recognition in `src/archive_layout.rs`. The verify path bypasses it and therefore disagrees with list/show, archive completion, task evidence, and dependency resolution about where one change may live.

## Proposed Solution

Introduce one proposal-location resolver for verification declarations:

- use the active proposal at `openspec/changes/<change-id>/proposal.md` whenever it exists, unconditionally and regardless of archive contents — an archive entry with the same ID is a previously archived change of that name, never a competing declaration source for a live one;
- otherwise resolve the archive entry through the existing archive-layout name rules (`archive_layout::is_valid_archive_entry_name`), which accept the direct `<change-id>` entry and the dated `YYYY-MM-DD-<change-id>` entry only, and additionally require `proposal.md` inside the entry;
- when more than one valid archive entry carries a `proposal.md` for the same change, fail closed rather than let `read_dir` order decide;
- when nothing valid resolves and `archive_layout::invalid_layout_error` reports a nested `YYYY-MM-DD/<change-id>` layout, surface that error's existing actionable message instead of a bare "no proposal" diagnostic;
- keep verification IDs and repository-relative `automation`, `evidence`, and `rerun` declarations unchanged across archive;
- keep the executor workspace exactly as it is today — `std::env::current_dir()` — so only the proposal read path changes, never the directory declared automation runs in.

Reuse the archive-layout name rules instead of adding an independent archive-name parser in `verify.rs`. Add the resolver as a **new sibling** of `archive_layout::find_valid_archive_entry` rather than tightening that function: its current behaviour — direct entry preferred, first `read_dir` match otherwise, `proposal.md` not required — is what `src/openspec_cmd.rs`, `src/task_file.rs`, and `src/execution/archive.rs` already depend on, so changing it in place would silently move archive completion detection and task-file resolution far outside this change.

Archive does not move verification evidence: sidecars live at `.cflx/verification-evidence/<verification-id>.json`, which is repository-level and keyed by verification ID, and the automation binding is a tracked blob OID for a repository-relative path. That is why an archived declaration keeps its identity — nothing about evidence binding has to move with the proposal.

## Acceptance Criteria

- `cflx openspec verify <change-id>` reads declarations from an active proposal exactly as before.
- An active proposal takes precedence over any archive entry with the same change ID, so a change that verifies today never starts failing because a same-named change was archived earlier.
- When no active proposal exists, the command reads equivalent declarations from the sole valid direct or dated archive entry that contains `proposal.md`.
- An archived declaration with repository-level automation can be planned, rerun, and captured using the same verification ID as before archive, against the unchanged `.cflx/verification-evidence` sidecar.
- More than one valid archive entry with `proposal.md` for the same change fails closed with an actionable diagnostic naming the competing entries, instead of depending on filesystem iteration order.
- Nested date directories, malformed dates, suffix collisions, archive entries without `proposal.md`, and unrelated archive entries never satisfy resolution, and a nested date layout reports the existing `archive_layout::invalid_layout_error` message.
- `archive_layout::find_valid_archive_entry` keeps its current behaviour, and every existing caller of it keeps its current results.
- Existing evidence binding, dirty-tree rejection, command supervision, executor workspace, and exit-status semantics remain unchanged.
- Focused tests exercise active, active-over-archive precedence, direct archive, dated archive, archive-only execution, duplicate archives, invalid nested layout, an entry without `proposal.md`, and missing change behavior.

## Explicit Completion Conditions

- `verify.rs` no longer constructs an active-only proposal path as its complete change-resolution policy.
- Archived declaration loading reuses `archive_layout`'s existing entry-name rules rather than a second parser.
- `archive_layout::find_valid_archive_entry` is unmodified and no existing caller of it changes behaviour.
- `cargo test --lib` passes.
- `cflx openspec validate verify-archived-change --strict --evidence error` passes.
- `cflx openspec validate verify-archived-change --archive-gate` passes before archive.

## Out of Scope

- Rewriting physical paths inside archived proposal frontmatter.
- Allowing automation inside a moving change-local subtree; a declaration whose automation path no longer exists after archive keeps reporting through the existing eligibility and evidence failure paths.
- Tightening `archive_layout::find_valid_archive_entry` or any other existing archive helper.
- Relocating or re-keying verification evidence sidecars.
- Changing evidence sidecar schema, reuse policy, or command supervision.
- Repairing malformed or duplicated archive directories automatically.
- Changing archive naming or migration policy.
