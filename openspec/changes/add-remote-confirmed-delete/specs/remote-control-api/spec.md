## ADDED Requirements

### Requirement: Confirmed destructive worktree deletion API

V2 MUST expose `GET /api/v2/worktrees/{worktree_id}/delete-preview` as a
read (no command record, no revision change, both listeners) that performs a
fresh targeted observation and returns `observed_at` (top level, outside
`consent`), `branch`, `head`, `is_main`, `dirty`, `has_commits_ahead`,
`inspection`, `content_observation` (`observed`, `status_failed`,
`unbounded`, `nested_repository`), `has_ignored_content`, `ignored_entries`,
`teardown_script_present` (true exactly when the teardown runner would
execute the script: a regular file with an execute bit), per-option
eligibility with a typed `blocked_reason` for `ordinary`, `skip_teardown`,
`discard_known_dirty`, and `discard_commits_ahead` (with
`also_discards_dirty`), and a `consent` object holding exactly `branch`,
`head`, `dirty`, `has_commits_ahead`, `has_ignored_content`, `merge_base`,
`discardable_content_id`, and `ignored_content_id`, which the command MUST
accept verbatim.

Both identities MUST come from one bounded read-only oracle over the sorted
records of `git status --porcelain=v2 -z --untracked-files=all
--ignored=traditional`, which is the exact on-disk file set that
`git worktree remove --force` deletes beyond `HEAD`. `discardable_content_id`
MUST be `sha256:` over the staged, unstaged, unmerged, and untracked records
combined with the no-follow on-disk bytes (or symlink target, or deletion
marker) of every path with a worktree-side difference and MUST change
whenever any such byte changes. `ignored_content_id` MUST be `sha256:` over
every ignored path's existence, file type, size, mtime, and mode, read with
`lstat` only, and MUST change whenever an ignored path is added, modified,
or removed, while `dirty` keeps its `--ignored=no` meaning. Files MUST be
opened `O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC` and re-checked with `fstat` so that
a FIFO or a path swapped after `lstat` is recorded by type without being
read or followed. The oracle MUST have documented finite record and byte
limits beyond which the observation is `unbounded` with no partial identity,
MUST report a directory record or a status-reported submodule as
`nested_repository`, MUST publish both identities and `has_ignored_content`
as `null` with the typed `content_observation` whenever it is not
`observed`, and MUST be computed through the read-only status policy
without writing, staging, running hooks, or excluding any path.

V2 MUST extend the closed command set with `delete_worktree_confirmed`
(target opaque `worktree_id`) whose typed params are the three independent
booleans `skip_teardown`, `discard_known_dirty`, `discard_commits_ahead` and
the echoed `consent`, with `deny_unknown_fields`; either discard permission
without both `discardable_content_id` and `ignored_content_id` (regardless of
`consent.dirty`), `discard_commits_ahead` without `merge_base`, or a
non-40-hex commit MUST be `validation_failed`. Under the repository root
mutation guard the shared service MUST re-observe the target, including the
content observation, and refuse with `delete_consent_stale` (reasons
`branch_changed`, `identity_changed`, `head_moved`, `base_moved`,
`dirty_changed`, `ahead_changed`, `ignored_changed`, `content_changed`) when
any consented fact or either recomputed identity differs, MUST refuse
unknown dirty and an unobservable or unbounded content observation as
`worktree_dirty_unknown`, MUST refuse unknown ahead, the main worktree, and
a `nested_repository` observation as `target_ineligible` whatever
permissions were given, MUST refuse known ignored content without
`discard_known_dirty` as `worktree_dirty` with reason `ignored_content`
even when `dirty` is `false`, MUST refuse a held guard or base merge as
`root_busy`, and MUST apply the unchanged eligibility, teardown, drift,
branch-ref, removal, and branch-cleanup steps with the `DeleteOptions` the
permissions map to. The service MUST re-run the content observation after
teardown and refuse `delete_consent_stale` with `teardown_ran: true` without
removing anything when `discardable_content_id` changed or when the ignored
identity is neither the consented one nor the empty-set identity. Nothing
MUST be removed on any refusal.

The settled result kind `delete_worktree_confirmed` MUST report `teardown`
(`ran`, `skipped`, `absent`, `not_reached`), `dirty_discarded`,
`commits_discarded`, `branch_deleted`, `branch_retained_reason`, and, on
refusal, `refusal.reason`, `refusal.teardown_ran`, and the freshly observed
facts including both identities and `content_observation`. The `delete_worktree` command MUST remain unchanged (empty params,
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
- **AND** `consent.discardable_content_id` and `consent.ignored_content_id` are `sha256:` identities and `observed_at` is outside `consent`
- **AND** the `consent` object submitted verbatim as `params.consent` passes validation
- **AND** no command record exists and the revision is unchanged

#### Scenario: Ignored content is disclosed and needs dirty discard

- **GIVEN** a worktree that `git status --ignored=no` reports clean but which holds an ignored file
- **WHEN** an authenticated client reads its `delete-preview` and then submits `delete_worktree_confirmed` without `discard_known_dirty`
- **THEN** the preview publishes `dirty: false`, `has_ignored_content: true`, `ignored_entries`, and `ordinary` blocked with `ignored_content`
- **AND** the command refuses `worktree_dirty` with reason `ignored_content` and removes nothing
- **AND** modifying the ignored file after the preview makes a `discard_known_dirty: true` submission `delete_consent_stale` with `content_changed`
- **AND** fresh consent with `discard_known_dirty: true` removes the worktree

#### Scenario: Oracle limits and nested repositories fail closed

- **GIVEN** a worktree containing an untracked nested repository, or one whose status records exceed the documented limits
- **WHEN** an authenticated client reads its `delete-preview` and submits `delete_worktree_confirmed` with every permission
- **THEN** the preview publishes `content_observation` as `nested_repository` or `unbounded` with `null` identities and every option ineligible
- **AND** the command refuses `target_ineligible` (reason `nested_repository`) or `worktree_dirty_unknown` (reason `content_unbounded`)
- **AND** nothing is torn down or removed and `delete_worktree` behaves as before

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
- **AND** `discard_commits_ahead: true` with `consent.dirty: false` and no identities is `validation_failed` before any observation
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
