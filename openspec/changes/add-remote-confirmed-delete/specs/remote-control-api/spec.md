## ADDED Requirements

### Requirement: Confirmed destructive worktree deletion API

V2 MUST expose `GET /api/v2/worktrees/{worktree_id}/delete-preview` as a
read (no command record, no revision change, both listeners) that performs a
fresh targeted observation and returns `branch`, `head`, `is_main`, `dirty`,
`has_commits_ahead`, `inspection`, `teardown_script_present`, per-option
eligibility with a typed `blocked_reason` for `ordinary`, `skip_teardown`,
`discard_known_dirty`, and `discard_commits_ahead` (with
`also_discards_dirty`), and a `consent` object holding `branch`, `head`,
`dirty`, `has_commits_ahead`, `merge_base`, and `discardable_content_id`.
`discardable_content_id` MUST be `sha256:` over the sorted records of
`git status --porcelain=v2 -z --untracked-files=all` combined with the
no-follow on-disk bytes (or symlink target, or deletion marker) of every path
with a worktree-side difference, MUST change whenever any staged, unstaged,
or untracked byte changes, MUST be `null` when the observation fails, and
MUST be computed through the read-only status policy without writing,
staging, or running hooks.

V2 MUST extend the closed command set with `delete_worktree_confirmed`
(target opaque `worktree_id`) whose typed params are the three independent
booleans `skip_teardown`, `discard_known_dirty`, `discard_commits_ahead` and
the echoed `consent`, with `deny_unknown_fields`; a discard permission
without `discardable_content_id`, `discard_commits_ahead` without
`merge_base`, or a non-40-hex commit MUST be `validation_failed`. Under the
repository root mutation guard the shared service MUST re-observe the target
and refuse with `delete_consent_stale` (reasons `branch_changed`,
`identity_changed`, `head_moved`, `base_moved`, `dirty_changed`,
`ahead_changed`, `content_changed`) when any consented fact or the recomputed
content identity differs, MUST refuse unknown dirty as
`worktree_dirty_unknown` and unknown ahead or the main worktree as
`target_ineligible`, MUST refuse a held guard or base merge as `root_busy`,
and MUST apply the unchanged eligibility, teardown, drift, branch-ref,
removal, and branch-cleanup steps with the `DeleteOptions` the permissions
map to. A discard permission MUST re-verify the content identity after
teardown and refuse `delete_consent_stale` with `teardown_ran: true` without
removing anything when it changed. Nothing MUST be removed on any refusal.

The settled result kind `delete_worktree_confirmed` MUST report `teardown`
(`ran`, `skipped`, `absent`, `not_reached`), `dirty_discarded`,
`commits_discarded`, `branch_deleted`, `branch_retained_reason`, and, on
refusal, `refusal.reason`, `refusal.teardown_ran`, and the freshly observed
facts. The `delete_worktree` command MUST remain unchanged (empty params,
fail-closed options, existing refusals). Capabilities MUST advertise
`worktrees.confirmed_delete` (`available`, `command`, `preview_route`,
`permissions`, `consent_fields`, `content_id_algorithm`, `reasons`), MUST
keep `delete_requires_teardown: true` with its description narrowed to the
`delete_worktree` command, and MUST decode `confirmed_delete` as unavailable
when an older owner omits it. The generated OpenAPI document MUST include the
command variant, the preview route, the result kind, and the error code.
`cflx client` and `cflx client mcp` MUST gain no verb.

#### Scenario: Preview publishes facts, options, and consent

- **GIVEN** a dirty worktree with commits ahead of base and a `.wt/teardown` script
- **WHEN** an authenticated client reads its `delete-preview`
- **THEN** `dirty`, `has_commits_ahead`, `teardown_script_present`, and each option's eligibility and reason are published
- **AND** `consent.discardable_content_id` is a `sha256:` identity
- **AND** no command record exists and the revision is unchanged

#### Scenario: Skip-teardown removes without running the script

- **GIVEN** a clean worktree whose `.wt/teardown` writes a marker
- **WHEN** `delete_worktree_confirmed` is submitted with `skip_teardown: true` and valid consent
- **THEN** the worktree is removed, the marker is absent, the result reports `teardown: skipped`
- **AND** the opaque ID is retired

#### Scenario: Same HEAD and same dirty flag but changed content is stale

- **GIVEN** a preview taken while `README` holds `A` with `dirty: true`
- **WHEN** `README` is changed to `B` without committing and the consent is submitted with `discard_known_dirty: true`
- **THEN** the command fails `delete_consent_stale` with reason `content_changed`
- **AND** the worktree, branch, and ID are intact and `refusal.observed` carries the new identity

#### Scenario: Ahead and dirty requires both permissions

- **GIVEN** a worktree that is dirty and ahead of base
- **WHEN** consent is submitted with `discard_commits_ahead: true` and `discard_known_dirty: false`
- **THEN** the command refuses `worktree_dirty` and removes nothing
- **AND** with both permissions the worktree is removed, the branch is force-deleted at the consented commit, and the result reports both discards

#### Scenario: Cancel submits nothing

- **GIVEN** a client read the preview
- **WHEN** the operator cancels and no command is submitted
- **THEN** the worktree, its ID, the command registry, and the revision are unchanged

#### Scenario: Teardown failure is recoverable by consented skip

- **GIVEN** a worktree whose `.wt/teardown` exits non-zero
- **WHEN** an ordinary delete fails and a `delete_worktree_confirmed` with `skip_teardown: true` and fresh consent follows
- **THEN** the first fails `internal_error` retaining the worktree and its ID
- **AND** the second removes the worktree and reports `teardown: skipped`

#### Scenario: Unknown, main, and busy fail closed

- **GIVEN** a target whose dirty state cannot be observed, the main worktree, or a held root guard
- **WHEN** `delete_worktree_confirmed` is submitted
- **THEN** the refusals are `worktree_dirty_unknown`, `target_ineligible`, and `root_busy` respectively
- **AND** nothing is torn down or removed

#### Scenario: Ordinary delete and older owners are unchanged

- **GIVEN** an owner built before this surface and a current owner
- **WHEN** `delete_worktree` is submitted with a `skip_teardown` field, and `delete_worktree_confirmed` is submitted to the older owner
- **THEN** both are `validation_failed` with no delegation
- **AND** `delete_requires_teardown` is published as `true` by both owners
- **AND** `confirmed_delete` decodes as unavailable from the older owner
