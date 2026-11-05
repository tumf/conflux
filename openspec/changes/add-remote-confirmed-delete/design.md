# Design: confirmed destructive worktree deletion

## Source evidence at develop `62d256107`

- `DeleteOptions { skip_teardown, allow_known_dirty, allow_commits_ahead }`
  with `fail_closed()`, `local()`, `local_discarding_dirty()`,
  `local_discarding_ahead()` (`src/worktree_ops/service.rs:331-404`).
- `classify_delete_eligibility`: main → `Ineligible`; base merge in progress
  → `RootBusy` / unknown → `Ineligible`; ahead without permission →
  `CommitsAhead { AheadTarget }` (or `DirtyUnknown` when dirty is unknown);
  ahead unknown → `Ineligible`; dirty without permission → `Dirty { DirtyTarget }`;
  dirty unknown → `DirtyUnknown` (`service.rs:412-470`).
- `ExpectedTarget { branch, identity, head }` and `classify_delete_identity`;
  `classify_delete_drift` compares path, identity, branch, head, and the
  three tri-state facts across teardown (`service.rs:484-620`).
- `delete_worktree`: root guard → locate → identity → eligibility → teardown
  unless skipped → locate → identity → drift → eligibility →
  `confirm_branch_ref` → remove → `discard_ahead_branch` or `cleanup_branch`
  → events (`service.rs:995-1070`).
- Remote port: `delete` resolves the opaque ID to `(path, identity)`, passes
  `ExpectedTarget::unchecked().with_identity(..)` and `fail_closed()`, retires
  the ID (`src/web/remote_control_api/worktrees.rs:540-568`).
- Capability `delete_requires_teardown: true`, doc "True when deletion always
  runs managed teardown with no bypass" (`worktrees.rs:384-391`).
- `.wt/teardown` runs from the worktree only when it is a regular executable
  file (`src/vcs/git/commands/worktree.rs:1003-1045`).
- Read-only status policy: every production `git status` must be built with
  `read_only_status_argv` and is inventoried by
  `src/vcs/git/commands/status_policy_repository_tests.rs:600-660`. The
  dirty observation uses `DIRTY_STATE_STATUS_ARGS` with `--ignored=no`
  (`status_policy.rs:51-52`), so `dirty` never reflects ignored paths.
- Removal is `worktree_remove_forced` → `git worktree remove <path> --force`
  (`src/vcs/git/commands/worktree.rs:673-677`), which deletes the whole
  directory including ignored paths that `dirty` does not see.
- `git status` never lists a `.git` entry, an empty directory, or an entry
  that is neither a regular file, a directory, nor a symlink, in any
  untracked or ignored mode. Observed on Git 2.53 in a scratch repository:
  with `--untracked-files=all --ignored=traditional` a gitfile at
  `target/wt/.git`, a FIFO at `target/deep/fifo`, and an ignored directory
  holding only an empty subdirectory produce no record, while a nested
  repository at `target/nested` is reported as the directory record
  `! target/nested/`. With `--untracked-files=normal --ignored=traditional`
  an all-untracked directory is one `? dir/` record and an all-ignored
  directory is one `! dir/` record, neither descended into. The oracle
  therefore walks collapsed directory records itself (below) instead of
  trusting Git's expansion to enumerate the deleted set.
- `DeleteOptions` doc comments (`service.rs:332-350`) currently say
  `skip_teardown` is "never reachable from `/api/v2`" and that only the local
  TUI may set the two discard permissions; this change makes those sentences
  false and must rewrite them.
- TUI: `DeleteIntent` carries the three permissions separately
  (`src/tui/types.rs:262-300`); the handler escalates `Dirty`/`CommitsAhead`
  refusals into confirmations (`src/tui/command_handlers.rs:531-596`).

## Contract

### Preview resource

`GET /api/v2/worktrees/{worktree_id}/delete-preview` (bearer-gated, both
listeners, headless `cflx run` included; no record, no revision change).
It performs `ObservationRequest::Target(path)` (fresh ahead/conflict/dirty
inspection, exactly what the mutation path uses) and computes the consent
identity. Response `DeletePreviewResponse`:

```json
{"instance_id": "…", "state_revision": 41, "worktree_id": "…",
 "observed_at": "RFC3339",
 "branch": "feature-x", "head": "<40 hex>", "is_main": false,
 "dirty": true, "has_commits_ahead": true, "inspection": "checked",
 "content_observation": "observed",
 "has_ignored_content": true, "ignored_entries": 3,
 "teardown_script_present": true,
 "options": {
   "ordinary":              {"eligible": false, "blocked_reason": "commits_ahead"},
   "skip_teardown":         {"eligible": false, "blocked_reason": "commits_ahead"},
   "discard_known_dirty":   {"eligible": false, "blocked_reason": "commits_ahead", "entries": 2},
   "discard_commits_ahead": {"eligible": true,  "blocked_reason": null, "commits": 3, "also_discards_dirty": true}},
 "consent": {"branch": "feature-x", "head": "<40 hex>", "dirty": true,
             "has_commits_ahead": true, "has_ignored_content": true,
             "merge_base": "<40 hex>",
             "discardable_content_id": "sha256:…",
             "ignored_content_id": "sha256:…"}}
```

`observed_at` is a top-level timing observation only. It is *not* a consent
field: `consent` carries exactly the eight fields the capability's
`consent_fields` lists, and the Server echoes that object byte-for-byte, so
a verbatim echo never trips `deny_unknown_fields`.

Option eligibility is `classify_delete_eligibility` evaluated with the
option's `DeleteOptions` (`ordinary` = `fail_closed`, `skip_teardown` =
`local(true)`, `discard_known_dirty` = `local_discarding_dirty(false)`,
`discard_commits_ahead` = `local_discarding_ahead(false, dirty == Dirty)`),
with `blocked_reason` from the closed vocabulary below, then narrowed by the
content observation:

- `content_observation` ≠ `observed` → every option is ineligible with that
  outcome as `blocked_reason` (`content_unobservable`, `content_unbounded`,
  `nested_repository`), because the command refuses it regardless of
  permissions.
- `has_ignored_content: true` → `ordinary` and `skip_teardown` are ineligible
  with `blocked_reason: ignored_content`; `discard_known_dirty` is eligible
  even when `dirty` is `false`, because `git worktree remove --force` deletes
  ignored paths and that loss needs the same explicit permission as known
  uncommitted work. `dirty` itself keeps its `--ignored=no` meaning.

`entries` is the number of paths in the discardable identity;
`ignored_entries` is the number of paths in the ignored identity (ignored
status records plus every entry walked beneath an ignored directory record,
directories included); `commits` is
`git rev-list --count merge_base..head`. `teardown_script_present` is `true`
exactly when the runner would execute `.wt/teardown`: the path resolves,
through the same follow-symlink `exists`/`is_file`/`metadata` checks
`run_worktree_teardown` makes (`worktree.rs:1003-1045`), to a regular file
with at least one execute bit; a directory, a dangling link, or a
non-executable file is `false` and yields `teardown: absent`.
An unknown or retired ID is `worktree_not_found`.

### Discardable content identity

`src/worktree_ops/consent.rs::observe_discardable_content(worktree, limits: &ConsentLimits) -> Result<ContentObservation>`
where `ContentObservation` is `Observed { discardable_content_id, ignored_content_id, has_ignored_content, entries, ignored_entries }`
or a typed non-observation `Unobservable`, `Unbounded`, `NestedRepository`.
The non-observation variants carry no identity field at all, so a partial
or substitute identity cannot exist in the type. `ConsentLimits
{ max_records, max_hashed_bytes }` has one production value,
`ConsentLimits::PRODUCTION = { 500_000, 256 MiB }`, used by the preview and
the service alike; `WorktreeService` holds its limits and a test
constructor injects smaller ones, so a limit case needs no multi-hundred-
megabyte fixture.

Both identities bind **bytes**. Metadata (existence, size, mtime, mode) is
bound in addition, never instead: a same-size rewrite of a file with its
mtime and mode restored leaves every metadata field intact and changes the
identity only because the bytes are hashed. That is the property the
previously agreed `content_changed` refusal promises, and it holds for
ignored content exactly as for tracked and untracked content.

1. **Status.** Run `git --no-optional-locks status --porcelain=v2 -z
   --untracked-files=normal --ignored=traditional` via a new
   `CONSENT_STATUS_ARGS` in `status_policy.rs` (added to the inventory
   test). In this mode Git reports a directory whose entries are all
   untracked as one `? dir/` record, a directory whose entries are all
   ignored as one `! dir/` record, and a nested repository as a directory
   record, without descending into any of them. `--untracked-files=all` is
   deliberately not used: Git's expansion hides `.git` entries, empty
   directories, and special files (source evidence above), so an expanded
   listing is not the set `git worktree remove --force` deletes. Any status
   failure or unparsable record → `Unobservable`, never an empty identity.
2. **Record kinds.** Each record is ordinary `1`, rename/copy `2`,
   unmerged `u`, untracked `?`, or ignored `!`. The verbatim fields (XY,
   submodule state, modes `mH mI mW`, index/HEAD OIDs, score, path and
   original path) are collected for the non-ignored kinds; `!` records
   contribute the path. A record whose submodule field starts with `S` (a
   status-reported submodule) → `NestedRepository`.
3. **Every open is fd-relative and no-follow.** The worktree root is opened
   once with `open(O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_CLOEXEC)` and every
   path from then on is resolved one component at a time from that
   descriptor with `openat(dirfd, component,
   O_RDONLY|O_DIRECTORY|O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC)` for directories,
   confirmed by `fstat`; the leaf is examined with `fstatat(dirfd, name,
   AT_SYMLINK_NOFOLLOW)`. Nothing is `chdir`ed and no absolute path is
   re-resolved, so a component swapped for a symlink after the preview
   cannot redirect the oracle, and no symlink is ever traversed. These are
   `libc` calls already available to the crate; no new dependency.
4. **Leaf identity**, the same for every kind of record and for every walked
   entry (`<p>` is the record prefix: nothing for discardable paths, `i:`
   for ignored ones):
   - regular file → `openat(dirfd, name, O_RDONLY|O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC)`,
     then `fstat` the open descriptor: still a regular file →
     `<p>f:<mode>:<size>:<mtime_ns>:sha256(bytes)` read to EOF; anything
     else (swapped for a FIFO, device, socket, or directory between
     `fstatat` and `open`) → `<p>o:<type from fstat>` without reading. An
     `open` that fails with `ELOOP` → `<p>o:symlink`; `ENXIO` →
     `<p>o:socket`. The non-blocking open is what prevents a FIFO from
     parking the owner under the root guard; the post-open `fstat` closes
     the `fstatat`→`open` race;
   - symlink → `<p>l:sha256(readlinkat target)`: the target string's bytes,
     never followed;
   - directory → `<p>dir:<mode>` and the walk of step 5;
   - FIFO, socket, device, or any other type → `<p>o:<type>`, never opened;
   - missing (a status-reported path raced away) → `<p>d`;
   - any other error (`EACCES`, `EMFILE`, `EIO`, …) → `Unobservable`.
5. **Directory records are walked, never trusted and never refused on
   sight.** A `?` or `!` record whose path ends in `/`, and every directory
   met beneath it, is enumerated with `fdopendir` on the descriptor from
   step 3 and each entry is classified by step 4 with `fstatat(dirfd, name,
   AT_SYMLINK_NOFOLLOW)`. An entry named `.git` — directory or file, valid
   gitfile or not — at any depth → `NestedRepository`: the content below it
   cannot be enumerated from this worktree's status and is not discarded.
   A directory is therefore not a nested repository because it is a
   directory; `! target/` is walked and bound, and only an actual `.git`
   descendant (or the `S` submodule record of step 2) refuses. Every walked
   directory is its own record, so an empty subdirectory is bound exactly
   (its creation or removal changes the identity). A symlink to a directory
   is a symlink record; the walk does not enter it, so content behind it is
   neither bound nor read. Directory descriptors are closed on ascent; the
   record cap below bounds the walk, and descriptor exhaustion is
   `Unobservable` like any other walk error.
6. **Membership: each path in exactly one identity.** A path belongs to the
   ignored identity when status reported it as `!` or it was walked beneath
   a `!` directory record; every other record and every entry walked beneath
   a `?` directory record belongs to the discardable identity. Git reports
   an ignored entry inside an all-untracked directory as both `? dir/` and
   `! dir/file` (or `! dir/sub/`); that path, with its subtree for a
   directory, is ignored content, is skipped by the enclosing `dir/` walk,
   and is read once through its own record. No path is read twice and no
   path is excluded.
7. **Bound: one combined budget.** `records` counts status records plus
   every walked entry (directories, files, symlinks, specials) across
   tracked, untracked, and ignored content together; more than
   `max_records` → `Unbounded`. `hashed_bytes` counts regular-file bytes
   and symlink target bytes, each path once, across the same combined set;
   more than `max_hashed_bytes` → `Unbounded`. Both are checked before each
   read, a single file larger than the remaining byte budget is `Unbounded`
   with no partial hash, and `Unbounded` stops the walk. `Unbounded` is
   final for that observation: the oracle never substitutes a metadata-only
   identity, never hashes a prefix, never drops ignored content to fit, and
   never reports `observed` over a subset. Staying available is not a goal
   the oracle trades safety for. A worktree past the budget — typically a
   large `target/` or other build output — is not deletable through the
   confirmed command: the operator cleans that output in place, runs the
   project's own clean or `.wt/teardown` locally, or deletes through the
   TUI, which is the existing local operation, and previews again. This
   change adds no remote authority over content the oracle could not bind.
8. **Hash.** Sort records by path, join each record's fields with `\x1f`
   and records with `\x1e`, SHA-256 the bytes. `discardable_content_id` is
   `sha256:<hex>` over the discardable set and `ignored_content_id` is
   `sha256:<hex>` over the ignored set of step 6; `has_ignored_content` is
   whether the ignored set is non-empty.
9. **What no mode can list, stated exactly.** A directory that holds no
   regular file or symlink at any depth (an empty directory, or one holding
   only FIFOs, sockets, or devices) and a lone special file produce no
   status record in any untracked or ignored mode. They are bound as
   records only when they lie beneath a walked directory record; elsewhere
   they are neither disclosed nor bound, and they hold no file bytes. Every
   regular file and symlink that `git worktree remove --force` deletes
   beyond `HEAD` is either a status record or beneath a walked directory
   record, with the one existing exception of a populated submodule that
   status does not report (guarantee boundary below).

A clean worktree (no records) has the identity of the empty record set for
both IDs, not `null`. The preview publishes `content_observation` as
`observed`, `status_failed`, `unbounded`, or `nested_repository`; the two IDs
and `has_ignored_content` are `null` unless it is `observed`. The oracle
reads only; it never stages, writes, or runs hooks, and it excludes no path
(Constitution 4). The same function runs in the preview, again under the
root guard immediately before teardown, and again after teardown.

### Consent command

```json
{"type": "delete_worktree_confirmed",
 "target": {"worktree_id": "…"},
 "params": {"skip_teardown": true, "discard_known_dirty": true, "discard_commits_ahead": false,
            "consent": {"branch": "feature-x", "head": "<40 hex>", "dirty": true,
                        "has_commits_ahead": false, "has_ignored_content": false,
                        "merge_base": "<40 hex>",
                        "discardable_content_id": "sha256:…",
                        "ignored_content_id": "sha256:…"}},
 "expected_revision": 41, "idempotency_key": "…"}
```

`ConfirmedDeleteParams` and `DeleteConsent` carry `deny_unknown_fields`;
`DeleteConsent` has exactly the eight fields above, with
`has_ignored_content`, `merge_base`, `discardable_content_id`, and
`ignored_content_id` nullable so a preview's `consent` echoes verbatim
whatever the observation outcome was. `validate()` refuses
(`validation_failed`): blank `branch`; `head` or `merge_base` not 40
lowercase hex; either identity present but not `^sha256:[0-9a-f]{64}$`;
**either** `discard_known_dirty` or `discard_commits_ahead` true without
both `discardable_content_id` and `ignored_content_id` (the rule does not
depend on `consent.dirty`, so `discard_commits_ahead: true, dirty: false`
with a missing identity is 422, never an undefined comparison);
`discard_commits_ahead` true without `merge_base`. All three booleans
`false` is allowed: it is an ordinary delete with consent revalidation.

Execution, in `WorktreeService::delete_worktree_with_consent(path, expected, consent, options)`
where `expected = ExpectedTarget::on_branch(consent.branch).with_identity(registry identity).with_head(consent.head)`
and `options = DeleteOptions { skip_teardown, allow_known_dirty: discard_known_dirty, allow_commits_ahead: discard_commits_ahead }`:

1. Root guard (`root_busy` when held).
2. `locate` (targeted observation); `classify_delete_identity` → a
   mismatch is `delete_consent_stale` with reason `branch_changed`,
   `identity_changed`, or `head_moved`.
3. Consent facts: `dirty` and `has_commits_ahead` must be *known* and equal
   to the consent (`dirty_unknown` → `worktree_dirty_unknown`;
   `ahead_unknown` → `target_ineligible`; inequality →
   `delete_consent_stale` with reason `dirty_changed` / `ahead_changed`).
   Content observation, always (one status call): `Unobservable` →
   `worktree_dirty_unknown` reason `content_unobservable`; `Unbounded` →
   `worktree_dirty_unknown` reason `content_unbounded`; `NestedRepository`
   → `target_ineligible` reason `nested_repository`, whatever the
   permissions. Then `has_ignored_content` must equal the consent
   (`delete_consent_stale` reason `ignored_changed`; a `null` consent value
   never equals an observed one); `has_ignored_content: true` without
   `discard_known_dirty` → `worktree_dirty` reason `ignored_content`.
   Whenever the consent carries an identity (always, when any discard
   permission is set): recomputed `discardable_content_id` or
   `ignored_content_id` unequal → `delete_consent_stale` reason
   `content_changed`. When `discard_commits_ahead`: recompute `merge_base`;
   inequality → `delete_consent_stale` reason `base_moved`.
4. `classify_delete_eligibility(facts, options)` — unchanged refusals
   (`target_ineligible` for main/ahead-without-permission, `worktree_dirty`,
   `worktree_dirty_unknown`, `root_busy`).
5. Teardown unless `skip_teardown` (an absent or non-executable script is
   `teardown: absent`); `locate`; identity; `classify_delete_drift`;
   eligibility; re-run the content observation and apply the step 3 rules
   again with `teardown_ran: true` (worktree retained — teardown effects are
   not reversed and the record says so), with one allowance: teardown
   exists to clean build output, so the ignored identity after teardown
   must equal the consented one **or** be the empty-set identity. Ignored
   paths that teardown added or modified, or a partial cleanup that left a
   different ignored set, is `content_changed` with `teardown_ran: true`;
   the operator re-previews and recovers with `skip_teardown: true`.
   `discardable_content_id` has no such allowance: tracked or untracked
   content teardown touched is `content_changed`.
6. `confirm_branch_ref`; `remove_worktree`; `discard_ahead_branch` when
   `allow_commits_ahead` else `cleanup_branch`; events; retire the ID.

Every refusal before step 6 removes nothing and retains the ID binding.

### Result kind, errors, capability

```json
{"kind": "delete_worktree_confirmed", "worktree_id": "…", "branch": "feature-x", "head": "<40 hex>",
 "outcome": "deleted" | null,
 "teardown": "ran" | "skipped" | "absent" | "not_reached",
 "dirty_discarded": true, "commits_discarded": false,
 "branch_deleted": true, "branch_retained_reason": null,
 "refusal": null | {"reason": "content_changed", "teardown_ran": false,
                    "observed": {"branch": "…", "head": "…", "dirty": true, "has_commits_ahead": false,
                                 "has_ignored_content": false, "merge_base": "…",
                                 "discardable_content_id": "sha256:…", "ignored_content_id": "sha256:…",
                                 "content_observation": "observed"}}}
```

A refused record carries the same kind so a client can re-render the
confirmation from `refusal.observed` without a second preview. Error code
`DeleteConsentStale` (`delete_consent_stale`, HTTP 409). Reason vocabulary
(`ConfirmedDeleteReason`): `branch_changed`, `identity_changed`,
`head_moved`, `base_moved`, `dirty_changed`, `ahead_changed`,
`ignored_changed`, `content_changed`, `dirty_unknown`, `ahead_unknown`,
`content_unobservable`, `content_unbounded`, `nested_repository`,
`main_worktree`, `root_busy`, `commits_ahead`, `dirty`, `ignored_content`,
`teardown_failed`. No new error code beyond `delete_consent_stale`: the
unknown and unbounded outcomes use `worktree_dirty_unknown`, the nested
repository uses `target_ineligible`, and undeclared ignored content uses
`worktree_dirty`, each with its typed reason in the result.

`CapabilitiesResponse.worktrees` gains (`#[serde(default)]`):

```json
"confirmed_delete": {"available": true, "command": "delete_worktree_confirmed",
  "preview_route": "/api/v2/worktrees/{worktree_id}/delete-preview",
  "permissions": ["skip_teardown", "discard_known_dirty", "discard_commits_ahead"],
  "consent_fields": ["branch", "head", "dirty", "has_commits_ahead", "has_ignored_content",
                     "merge_base", "discardable_content_id", "ignored_content_id"],
  "content_id_algorithm": "sha256",
  "binds_ignored_content": true,
  "reasons": ["branch_changed", "…"]}
```

`delete_requires_teardown` stays `true`; its schema description becomes
"True when the `delete_worktree` command always runs managed teardown with no
bypass. The only bypass is the separately advertised `confirmed_delete`
command, which requires explicit consent." The `operations` list gains
`"delete_confirmed"`. `WorktreeEligibility` is unchanged: per-option
eligibility is on the preview, which is where the fresh inspection runs.

### Guarantee boundary

Between the preview and the command nothing is locked. The guarantee is:
the deletion proceeds only if, under the root guard immediately before
teardown and again immediately before removal, every consented fact and the
recomputed content identities still equal what the client echoed; otherwise
nothing is removed. External writes landing after the final comparison and
before `git worktree remove --force` are not detected; this window is the
same one the TUI has, bounded by the root guard excluding Conflux's own
operations, and the usage guide says so.

What the identities cover, stated so the usage guide can repeat it:

- Tracked modifications, staged changes, unmerged paths, untracked files,
  and ignored files are all bound by their bytes (or symlink target string),
  with type, size, mtime, and mode bound in addition. Metadata is never the
  proof: a same-size rewrite with mtime and mode restored changes the
  identity. Ignored content is disclosed as `has_ignored_content` /
  `ignored_entries`; it is deleted by `git worktree remove --force` and
  therefore requires `discard_known_dirty` even when `dirty` is `false`.
  `dirty` keeps its `--ignored=no` meaning; the ignored fact is a separate,
  additional one.
- Collapsed directories (`? dir/`, `! dir/`) are walked by the oracle, so
  files at any depth, empty subdirectories, special files, and symlinks
  beneath them are bound. Empty directories and lone special files that lie
  under no walked record are listed by no status mode, hold no bytes, and
  are not bound; the design states this rather than claiming otherwise.
- Content under an actual nested repository (a `.git` descendant) or a
  status-reported submodule is never bound; the confirmed command fails
  closed with `nested_repository`. A populated submodule that status does
  not report is outside the oracle; Git's own `worktree remove` guard
  refuses such a worktree, which the service surfaces as `internal_error`
  with the worktree retained, exactly as `delete_worktree` does today.
- Past the combined record or byte budget (tracked, untracked, and ignored
  together, each path once) the observation is `unbounded` and the confirmed
  command refuses whatever permissions were given; no metadata-only or
  partial identity is substituted to keep the command available. Large
  build output is reduced locally, not deleted remotely. The ordinary
  `delete_worktree` path is not weakened or changed by any of this.

### Constitution check

The oracle reads status and file bytes only; no file is created in the
worktree. Consent is request-scoped; nothing is stored. No scheduling or
resume decision reads any of this.

## Rejected alternatives

- Extending `delete_worktree` params: an older owner would reject the new
  fields safely, but `delete_requires_teardown: true` would then describe a
  command that can bypass teardown; a separate command keeps the published
  sentence true.
- Binding consent to `dirty`/`has_commits_ahead` booleans only: fails the
  `README A → B` condition.
- A server-issued opaque consent token: it would be owner-local state with a
  lifetime, and it hides from the client what it is consenting to; echoing
  the facts is auditable and stateless.
- `git stash` or a backup branch before discard: the TUI makes no such
  promise, and "discard" must mean discard.
- Binding ignored paths by metadata only (existence, size, mtime, mode): a
  same-size rewrite with the timestamp restored leaves every one of those
  fields intact, so the previously agreed `content_changed` refusal would
  become a false guarantee for ignored content. Bytes are hashed instead;
  metadata stays bound in addition.
- Falling back to a metadata identity, a prefix hash, or an ignored-content
  exclusion when the byte budget is exceeded: each keeps the command
  available by weakening what consent proves. `unbounded` is the truthful
  answer, and reducing a large build tree is a local operation that already
  exists.
- Trusting `--untracked-files=all` to enumerate collapsed directories: Git's
  expansion omits `.git` entries, empty directories, and special files, so
  it neither detects a nested repository hidden inside `target/` nor binds
  the whole deleted set. The oracle walks directory records itself with
  fd-relative no-follow calls.
- Refusing every directory record as a nested repository: with collapsed
  directory records that would make every worktree with a `target/`
  undeletable through this command. Only an actual `.git` descendant or a
  status-reported submodule is a nested repository.
- Hiding the ignored-path loss in documentation only: `git worktree remove
  --force` deletes those paths, so an undisclosed, unconsented loss would
  contradict the discard requirement this change exists to satisfy.
- Allowing arbitrary ignored drift after teardown: teardown may clean
  build output (empty ignored set), but anything else it leaves behind is
  content nobody consented to discard.
- Changing the TUI confirmations in this change: they share the service and
  options; binding content identity there is a separate, smaller change.

## Verification design (future implementation, not executed at proposal time)

Prefix `remote_confirmed_delete`. Real-Git cases use
`#[cfg_attr(not(feature = "heavy-tests"), ignore)]` and the isolated fixture
pattern of `src/orchestration/proposal_refresh/tests.rs`; service decision
cases use the existing `FakeBackend` in `src/worktree_ops/service/tests.rs`.

### Service gate: `remote-confirmed-delete-service` (`src/worktree_ops/service/tests.rs` and `src/worktree_ops/consent.rs`)

- `remote_confirmed_delete_consent_mismatch_refuses_before_teardown`: table
  over branch, identity, head, dirty, ahead, merge_base, content → no
  `Teardown`/`RemoveWorktree` call, typed reason.
- `remote_confirmed_delete_unknown_facts_fail_closed`: dirty unknown, ahead
  unknown, content `Unobservable`, `Unbounded`, and `NestedRepository` →
  refusal with the typed reason, no mutation.
- `remote_confirmed_delete_permissions_map_to_options_exactly`: each boolean
  combination produces the matching `DeleteOptions` and the matching branch
  cleanup call (`DeleteBranchAt` only with `discard_commits_ahead`).
- `remote_confirmed_delete_post_teardown_content_drift_refuses`: a backend
  whose teardown changes the recomputed identity → refusal with
  `teardown_ran: true`, no `RemoveWorktree`.
- `remote_confirmed_delete_content_identity_is_byte_sensitive` (real Git in
  `consent.rs` tests): clean, modified, staged, untracked, symlink, deleted,
  rename; `README A` vs `B` differ; same bytes twice equal; status failure →
  `Unobservable`; the oracle writes nothing (directory digest unchanged).
- `remote_confirmed_delete_content_identity_binds_ignored_paths`
  (`consent.rs` tests): adding, modifying, and deleting an ignored file
  changes `ignored_content_id` and `has_ignored_content`, leaves
  `discardable_content_id` unchanged, and leaves `DirtyState` `Clean`;
  an ignored file inside an all-untracked directory (reported as both
  `? dir/` and `! dir/file`) is in the ignored identity only and is read
  once.
- `remote_confirmed_delete_same_size_bytes_mutated_with_mtime_restored_changes_identity`
  (`consent.rs` tests, real Git): an ignored `target/a.o` holding `AAAA`
  is observed; it is rewritten to `AAAB` (same size) and its mtime and mode
  are restored with `File::set_times` and `set_permissions`; `lstat`
  confirms size, mtime, and mode equal the originals; `ignored_content_id`
  differs, `discardable_content_id` is unchanged, `has_ignored_content`
  stays `true`, and `DirtyState` stays `Clean`. The same rewrite of tracked
  `README` (`A` → `B`, mtime restored; Git still reports the record because
  ctime moves) changes `discardable_content_id`. This is the test that a
  metadata-only identity cannot pass.
- `remote_confirmed_delete_ignored_directory_walk_binds_descendant_drift`
  (`consent.rs` tests, real Git): `target/` is confirmed to be the single
  collapsed record `! target/`; the observation is `Observed`, not
  `NestedRepository`; each of the following changes `ignored_content_id`
  while `discardable_content_id` and `DirtyState` `Clean` are unchanged: a
  same-size mtime-restored rewrite of `target/deep/deeper/b.o`, a new file
  three levels deep, a removed file, a new empty subdirectory, a removed
  empty subdirectory, a file replaced by a symlink, and a symlink whose
  target string changes. A symlink inside `target/` pointing at a sibling
  directory with files is bound by its target string only: modifying a
  file behind it leaves the identity unchanged and the walk terminates.
  `git init target/nested` and, separately, a gitfile written to
  `target/wt/.git` each yield `NestedRepository`.
- `remote_confirmed_delete_bound_exceeded_fails_closed` (`consent.rs`
  tests with injected `ConsentLimits`, plus the service and API cases
  below): on a Git-clean worktree the record cap exceeded by walked ignored
  entries alone, the byte cap exceeded by ignored bytes alone, a single
  ignored file larger than the remaining budget, and a tracked modification
  plus ignored bytes that each fit alone but exceed the combined budget
  together each yield `Unbounded`; the variant carries no identity, so no
  metadata-only or partial identity can be asserted into existence; the
  walk stops at the bound. `ConsentLimits::PRODUCTION` is asserted equal to
  `500_000` records and `256 MiB`. In `service/tests.rs` an `Unbounded`
  observation with all three permissions refuses `worktree_dirty_unknown`
  reason `content_unbounded` with no `Teardown`/`RemoveWorktree` call.
- `remote_confirmed_delete_oracle_is_nonblocking_and_bounded`
  (`consent.rs` tests): a tracked path replaced by a FIFO with no writer
  yields `o:fifo` without hanging (asserted by completion, not by a
  wall-clock threshold); a FIFO inside a walked ignored directory yields
  `i:o:fifo` without being opened; a nested untracked repository and a
  modified submodule yield `NestedRepository`; a tracked path whose parent
  component is swapped for a symlink after the status call is not followed.
- `remote_confirmed_delete_ignored_content_requires_dirty_discard`
  (`service/tests.rs`): `has_ignored_content: true`, `dirty: false`, no
  `discard_known_dirty` → `worktree_dirty` reason `ignored_content`, no
  `Teardown`/`RemoveWorktree`; with the permission the delete proceeds and
  reports `dirty_discarded: true`.
- `remote_confirmed_delete_post_teardown_ignored_cleanup_is_allowed`
  (`service/tests.rs`): a teardown that removes every ignored path leaves
  the delete eligible; one that leaves a different ignored set or touches
  tracked content refuses `content_changed` with `teardown_ran: true`.
- `remote_confirmed_delete_status_argv_is_inventoried`: the new
  `CONSENT_STATUS_ARGS` call site appears in the read-only inventory.

### API gate: `remote-confirmed-delete-api` (`src/web/remote_control_api/tests/mod.rs`)

New module `src/web/remote_control_api/tests/confirmed_delete_tests.rs`
registered in `tests/mod.rs`, over a real repository with a dedicated dummy
worktree per case and a `.wt/teardown` script that writes a marker:

- `remote_confirmed_delete_preview_is_a_read_with_fresh_inspection`.
- `remote_confirmed_delete_skip_teardown_removes_without_running_teardown`
  (marker absent) and `…_ordinary_runs_teardown` (marker present).
- `remote_confirmed_delete_discards_known_dirty`, `…_discards_commits_ahead`
  (branch gone, `git rev-parse --verify` fails), `…_discards_both_with_both_permissions`,
  `…_ahead_with_dirty_needs_both_permissions` (dirty refusal, nothing removed).
- `remote_confirmed_delete_same_head_same_dirty_content_drift_is_stale`:
  `README A`, preview, write `B`, submit → `delete_consent_stale`
  `content_changed`, worktree intact; also untracked file added, new commit,
  branch switched, recreated path.
- `remote_confirmed_delete_cancel_submits_nothing`: preview only → registry
  empty, revision unchanged, worktree intact.
- `remote_confirmed_delete_teardown_failure_then_skip_recovery`.
- `remote_confirmed_delete_refuses_main_busy_root_and_unknown`.
- `remote_confirmed_delete_ordinary_delete_is_unchanged`: existing
  `delete_worktree` with `skip_teardown` param → 422; fail-closed refusals
  unchanged.
- `remote_confirmed_delete_preview_consent_round_trips_verbatim`: the
  preview's `consent` JSON is placed unmodified into `params.consent` and
  reaches the service (no 422), including when the observation outcome left
  nullable fields `null`.
- `remote_confirmed_delete_ignored_content_is_disclosed_and_consented`:
  a Git-clean worktree with an ignored file publishes `dirty: false`,
  `has_ignored_content: true`, `ordinary` blocked `ignored_content`; the
  command without `discard_known_dirty` refuses `worktree_dirty`; an ignored
  file modified after the preview is `delete_consent_stale`
  `content_changed`; with fresh consent and the permission the worktree is
  removed.
- `remote_confirmed_delete_ignored_directory_drift_is_stale`: a Git-clean
  worktree with `target/deep/b.o`; preview; same-size rewrite with mtime
  restored; `discard_known_dirty: true` submission → `delete_consent_stale`
  `content_changed`, worktree intact, `refusal.observed.ignored_content_id`
  differs from the consented one.
- `remote_confirmed_delete_unbounded_observation_fails_closed`: a
  `WorktreeService` built with small injected `ConsentLimits` and a
  Git-clean worktree whose ignored files exceed them → preview
  `content_observation: unbounded`, `null` identities, every option
  `content_unbounded`; the command with all three permissions `true` →
  `worktree_dirty_unknown` reason `content_unbounded`, `teardown_ran:
  false`, worktree and ID intact, no marker written.
- `remote_confirmed_delete_nested_repository_fails_closed`: an untracked
  nested repository, and separately a nested repository inside the ignored
  `target/` → preview `content_observation: nested_repository`, every
  option ineligible, the command `target_ineligible` reason
  `nested_repository` with nothing torn down or removed.
- `remote_confirmed_delete_old_owner_capabilities_decode_as_unavailable`
  (in `dto_tests.rs`) and `…_commands_are_closed` (unknown field; missing
  identities with `discard_known_dirty`; `discard_commits_ahead: true`,
  `dirty: false`, missing identities; short head → 422).

### Contract gate: `remote-confirmed-delete-contract` (`tests/openapi_contract_tests.rs`)

- `remote_confirmed_delete_contract_is_published_end_to_end`: the command
  variant with its params schema, the preview route, the result kind, the
  error code, `capabilities.worktrees.confirmed_delete`,
  `delete_requires_teardown` still required and `true`; negative control
  removes the route and the result kind from a copy.

### Non-declared regression runs the implementer must still report

`cargo test --lib --features web-monitoring remote_worktree`, the TUI
`command_handlers` deletion tests, and `tests/client_cli_tests.rs` must pass
unchanged except the worktree capability `operations` assertion.

## Independent review launch contract (for the implementation worker)

Independent implementation reviews are launched exactly as follows and never
through the built-in Agent/Task tools or a direct `claude` session:

```text
PATH=/home/tumf/.local/bin:/home/tumf/.cargo/bin:/home/tumf/bin:/usr/local/bin:/usr/bin:/bin
HOME=/home/tumf
/home/tumf/.cargo/bin/agent-exec run --until 100 --stdin-file /tmp/<unique-UUID>.md -- \
  /home/tumf/.local/bin/claude-auto-fable --output-format stream-json --verbose \
  --include-partial-messages --dangerously-skip-permissions --print
```

- No `--model` flag. The review session is separate from the implementation
  session and reads only the assigned diff plus compatibility; it is not a
  repository audit.
- The worker returns the review job ID, the raw log path, the exact argv, the
  actually responding model, and the final answer. A running job is neither
  killed nor duplicated.
- The parent owns acceptance; a wrapper or `wait` exit status `0` is not
  proof. No push, release, install, owner replacement, or global
  configuration edit is performed by the worker or the reviewer.
