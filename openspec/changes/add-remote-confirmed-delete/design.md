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
  `src/vcs/git/commands/status_policy_repository_tests.rs:600-660`.
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
 "branch": "feature-x", "head": "<40 hex>", "is_main": false,
 "dirty": true, "has_commits_ahead": true, "inspection": "checked",
 "teardown_script_present": true,
 "options": {
   "ordinary":              {"eligible": false, "blocked_reason": "commits_ahead"},
   "skip_teardown":         {"eligible": false, "blocked_reason": "commits_ahead"},
   "discard_known_dirty":   {"eligible": false, "blocked_reason": "commits_ahead", "entries": 2},
   "discard_commits_ahead": {"eligible": true,  "blocked_reason": null, "commits": 3, "also_discards_dirty": true}},
 "consent": {"branch": "feature-x", "head": "<40 hex>", "dirty": true,
             "has_commits_ahead": true, "merge_base": "<40 hex>",
             "discardable_content_id": "sha256:…", "observed_at": "RFC3339"}}
```

Option eligibility is `classify_delete_eligibility` evaluated with the
option's `DeleteOptions` (`ordinary` = `fail_closed`, `skip_teardown` =
`local(true)`, `discard_known_dirty` = `local_discarding_dirty(false)`,
`discard_commits_ahead` = `local_discarding_ahead(false, dirty == Dirty)`),
with `blocked_reason` from the closed vocabulary below. `entries` is the
number of status entries; `commits` is `git rev-list --count merge_base..head`.
An unknown or retired ID is `worktree_not_found`.

### Discardable content identity

`src/worktree_ops/consent.rs::discardable_content_id(worktree) -> Result<Option<String>>`:

1. Run `git --no-optional-locks status --porcelain=v2 -z --untracked-files=all`
   via a new `CONSENT_STATUS_ARGS` in `status_policy.rs` (added to the
   inventory test). Any failure → `None` (unknown), never an empty identity.
2. For each entry (ordinary `1`, rename/copy `2`, unmerged `u`, untracked `?`),
   collect the verbatim status record fields (XY, submodule state, modes
   `mH mI mW`, index/HEAD OIDs, score, path and original path).
3. For each path whose worktree side differs (second XY character not `.`,
   or `?`, or `u`): `lstat` with no follow; regular file → `f:sha256(bytes)`;
   symlink → `l:sha256(target)`; missing → `d`; anything else → `o:<type>`.
   Files are read with `O_NOFOLLOW`; a file over 64 MiB contributes
   `big:<size>:<sha256 of first 1 MiB>` so the oracle stays bounded.
4. Sort records by path, join each record's fields with `\x1f` and records
   with `\x1e`, SHA-256 the bytes, publish `sha256:<hex>`.

A clean worktree (no entries) has the identity of the empty record set, not
`null`. `null` means "could not be observed" and makes every discard option
ineligible (`blocked_reason: dirty_unknown`). The oracle reads only; it never
stages, writes, or runs hooks. The same function runs in the preview and
again under the root guard immediately before teardown and again after
teardown when a discard permission is in use.

### Consent command

```json
{"type": "delete_worktree_confirmed",
 "target": {"worktree_id": "…"},
 "params": {"skip_teardown": true, "discard_known_dirty": true, "discard_commits_ahead": false,
            "consent": {"branch": "feature-x", "head": "<40 hex>", "dirty": true,
                        "has_commits_ahead": false, "merge_base": "<40 hex>",
                        "discardable_content_id": "sha256:…"}},
 "expected_revision": 41, "idempotency_key": "…"}
```

`ConfirmedDeleteParams` and `DeleteConsent` carry `deny_unknown_fields`.
`validate()` refuses (`validation_failed`): blank `branch`; `head` or
`merge_base` not 40 lowercase hex; `discardable_content_id` present but not
`^sha256:[0-9a-f]{64}$`; `discard_known_dirty` or
(`discard_commits_ahead` and `consent.dirty`) true without a
`discardable_content_id`; `discard_commits_ahead` true without `merge_base`.
All three booleans `false` is allowed: it is an ordinary delete with consent
revalidation.

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
   When any discard permission is set: recompute `discardable_content_id`;
   `None` → `worktree_dirty_unknown`; inequality → `delete_consent_stale`
   reason `content_changed`. When `discard_commits_ahead`: recompute
   `merge_base`; inequality → `delete_consent_stale` reason `base_moved`.
4. `classify_delete_eligibility(facts, options)` — unchanged refusals
   (`target_ineligible` for main/ahead-without-permission, `worktree_dirty`,
   `worktree_dirty_unknown`, `root_busy`).
5. Teardown unless `skip_teardown` (an absent or non-executable script is
   `teardown: absent`); `locate`; identity; `classify_delete_drift`;
   eligibility; when a discard permission is set, recompute the content
   identity and compare (`delete_consent_stale`, reason `content_changed`,
   `teardown_ran: true`, worktree retained — teardown effects are not
   reversed and the record says so).
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
                                 "merge_base": "…", "discardable_content_id": "sha256:…"}}}
```

A refused record carries the same kind so a client can re-render the
confirmation from `refusal.observed` without a second preview. Error code
`DeleteConsentStale` (`delete_consent_stale`, HTTP 409). Reason vocabulary
(`ConfirmedDeleteReason`): `branch_changed`, `identity_changed`,
`head_moved`, `base_moved`, `dirty_changed`, `ahead_changed`,
`content_changed`, `dirty_unknown`, `ahead_unknown`, `main_worktree`,
`root_busy`, `commits_ahead`, `dirty`, `teardown_failed`.

`CapabilitiesResponse.worktrees` gains (`#[serde(default)]`):

```json
"confirmed_delete": {"available": true, "command": "delete_worktree_confirmed",
  "preview_route": "/api/v2/worktrees/{worktree_id}/delete-preview",
  "permissions": ["skip_teardown", "discard_known_dirty", "discard_commits_ahead"],
  "consent_fields": ["branch", "head", "dirty", "has_commits_ahead", "merge_base", "discardable_content_id"],
  "content_id_algorithm": "sha256",
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
recomputed content identity still equal what the client echoed; otherwise
nothing is removed. External writes landing after the final comparison and
before `git worktree remove --force` are not detected; this window is the
same one the TUI has, bounded by the root guard excluding Conflux's own
operations, and the usage guide says so.

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
  unknown, content `None` → refusal, no mutation.
- `remote_confirmed_delete_permissions_map_to_options_exactly`: each boolean
  combination produces the matching `DeleteOptions` and the matching branch
  cleanup call (`DeleteBranchAt` only with `discard_commits_ahead`).
- `remote_confirmed_delete_post_teardown_content_drift_refuses`: a backend
  whose teardown changes the recomputed identity → refusal with
  `teardown_ran: true`, no `RemoveWorktree`.
- `remote_confirmed_delete_content_identity_is_byte_sensitive` (real Git in
  `consent.rs` tests): clean, modified, staged, untracked, symlink, deleted,
  rename; `README A` vs `B` differ; same bytes twice equal; status failure →
  `None`; the oracle writes nothing (directory digest unchanged).
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
- `remote_confirmed_delete_old_owner_capabilities_decode_as_unavailable`
  (in `dto_tests.rs`) and `…_commands_are_closed` (unknown field, missing
  content id with discard, short head → 422).

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
