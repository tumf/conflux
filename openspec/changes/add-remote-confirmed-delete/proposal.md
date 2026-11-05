---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/remote-worktree-operations/spec.md
  - openspec/specs/tui-worktree-view/spec.md
  - openspec/specs/vcs-worktree-operations/spec.md
  - docs/guides/USAGE.md
verifications:
  - id: remote-confirmed-delete-service
    requirement: Shared worktree service binds explicit skip-teardown, dirty-discard, and ahead-discard consent to branch, HEAD, identity, observed facts, discardable content identity, and ignored content identity through a bounded non-blocking oracle, and refuses on any drift, unknown or unbounded fact, nested repository, undeclared ignored content, main, or busy root
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/worktree_ops/service/tests.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-confirmed-delete-api
    requirement: Delete preview and consent command behave against real Git with ordinary delete unchanged, cancel submitting nothing, and every drift and recovery path typed
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/mod.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_confirmed_delete -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-confirmed-delete-contract
    requirement: Generated OpenAPI, capabilities, error codes, and result kind publish the confirmed-delete surface without redefining delete_requires_teardown, and older owners decode as unavailable
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/openapi_contract_tests.rs
    evidence: cargo test --features web-monitoring --test openapi_contract_tests remote_confirmed_delete -- --nocapture
    rerun: cargo test --features web-monitoring --test openapi_contract_tests remote_confirmed_delete -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add remote confirmed destructive worktree deletion

**Change Type**: implementation

## Problem / Context

Conflux-server (CFSRV-52, OP-30 / OP-31 / OP-32) needs the three TUI
deletion escalations the Web cannot reach: skip teardown (`S`), discard
known uncommitted changes (`X` in the dirty-discard confirmation), and
discard commits ahead of base including branch deletion (`X` in the
ahead-discard confirmation), as documented in `docs/guides/USAGE.md:101-143`
and implemented through `DeleteOptions::local*`, `DirtyTarget`, and
`AheadTarget` (`src/worktree_ops/service.rs:170-240, 331-404`).

At develop `62d256107`, `/api/v2` `delete_worktree` is deliberately
fail-closed: `DeleteOptions::fail_closed()`, `EmptyParams` with
`deny_unknown_fields`, dirty refused, ahead refused, unknown refused
(`src/web/remote_control_api/worktrees.rs:540-568`), and the capability
`delete_requires_teardown: true` states that remote deletion always runs
teardown with no bypass (`worktrees.rs:372-391`). The Conflux-side review
accepted the extension on conditions: ordinary deletion keeps its meaning
and safety; each destructive permission is an explicit, separate choice;
consent is bound to the identifier, branch, HEAD, dirty and ahead facts and
re-checked immediately before execution; unknown facts are never "safe";
skip-teardown keeps its recovery role; and `delete_requires_teardown` is not
silently redefined. The handoff adds one stronger condition: consent must
bind the *actual discardable content*, so a worktree whose `README` moved
from `A` to `B` under the same `HEAD` and the same `dirty: true` invalidates a
consent given for `A`. A second safety fact follows from the removal step
itself: `git worktree remove --force` deletes the whole directory, including
ignored paths that the `--ignored=no` dirty observation never sees, so a
worktree that Git calls clean can still lose on-disk content. The preview
must disclose that content and consent must bind it; it must not be hidden
in documentation.

## Proposed Solution

Add one preview read and one consent command, leaving `delete_worktree`
byte-for-byte as it is:

- `GET /api/v2/worktrees/{worktree_id}/delete-preview` performs a fresh
  targeted observation and returns the facts an operator must see before
  consenting: `observed_at` (top level, informational), `branch`, `head`,
  `is_main`, `dirty`, `has_commits_ahead`, `inspection`,
  `content_observation`, `has_ignored_content` with `ignored_entries`,
  whether a `.wt/teardown` script is present (a regular executable file,
  as the runner decides), and the eligibility of each option (`ordinary`,
  `skip_teardown`, `discard_known_dirty`, `discard_commits_ahead` with
  `also_discards_dirty`) with a typed blocked reason. It also returns the
  `consent` object the client must echo back verbatim, exactly the eight
  fields `branch`, `head`, `dirty`, `has_commits_ahead`,
  `has_ignored_content`, `merge_base`, `discardable_content_id`, and
  `ignored_content_id`. Both identities come from one bounded, read-only
  oracle over `git status --porcelain=v2 -z --untracked-files=all
  --ignored=traditional`, whose record set is the exact file set
  `git worktree remove --force` deletes beyond `HEAD`:
  `discardable_content_id` is `sha256:` over the staged, unstaged, unmerged,
  and untracked records plus the no-follow on-disk bytes of every differing
  path; `ignored_content_id` is `sha256:` over every ignored path's
  existence, size, mtime, and mode. Files are opened
  `O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC` and re-checked with `fstat`, so a FIFO
  or a swapped path can neither hang the owner nor be followed. Past
  documented record and byte limits the observation is `unbounded`; a
  nested repository or a status-reported submodule is `nested_repository`;
  both fail the confirmed command closed with a typed reason.
- `delete_worktree_confirmed` (target opaque `worktree_id`) takes typed
  `params`: three independent booleans `skip_teardown`,
  `discard_known_dirty`, `discard_commits_ahead`, and the echoed `consent`.
  Whenever either discard permission is `true`, both identities are
  required at validation. Known ignored content is a separate discardable
  fact: it leaves `dirty` untouched but requires `discard_known_dirty`,
  so a Git-clean worktree with ignored files refuses `worktree_dirty`
  (reason `ignored_content`) until that permission is given.
  Under the repository root guard the shared service re-observes the target,
  revalidates branch, Git identity, and HEAD, compares every consent fact
  including both recomputed identities, and only then applies the
  existing teardown, re-observation, drift, branch-ref confirmation, removal,
  and branch cleanup steps with the matching `DeleteOptions`. Any drift,
  unknown or unbounded fact, nested repository, main worktree, busy root, or
  missing permission refuses before anything is removed; a refusal after
  teardown reports that teardown ran. After teardown the ignored identity may
  only be unchanged or empty, because cleaning build output is what teardown
  is for; anything else is drift. Any mismatch is the typed
  `delete_consent_stale` with the fresh facts.
- The capability block `confirmed_delete` advertises the command, the
  preview route, the three permissions, the consent fields, and the reason
  vocabulary. `delete_requires_teardown` keeps the value `true` and its
  published description is narrowed to the `delete_worktree` command it has
  always described, so no client reads a changed meaning. The worktree
  `operations` list gains `delete_confirmed`.
- The TUI's three confirmations are unchanged; they share `DeleteOptions`
  and the service steps with the new command.

## Acceptance Criteria

1. `delete_worktree` is unchanged: `EmptyParams`, fail-closed options,
   `worktree_dirty`, `worktree_dirty_unknown`, and `target_ineligible`
   refusals, and the existing tests pass without modification; a
   `delete_worktree` request carrying `skip_teardown` or any discard field
   is still `validation_failed` with no delegation.
2. The preview is a read: no command record, no revision change, a fresh
   targeted observation, facts and per-option eligibility that agree with
   what the command would decide, `observed_at` outside `consent`, and a
   `consent` object that echoes verbatim without `validation_failed`, whose
   `discardable_content_id` changes when any staged, unstaged, or untracked
   byte changes, whose `ignored_content_id` and `has_ignored_content` change
   when any ignored path is added, modified, or removed while `dirty` stays
   `false`, and whose identities are `null` with a typed
   `content_observation` when the oracle cannot observe, exceeds its limits,
   or meets a nested repository.
3. Against a real Git fixture: skip-teardown removes without running
   `.wt/teardown` (marker absent) while ordinary runs it (marker present);
   dirty discard removes a modified worktree and its clean branch; ahead
   discard removes the worktree and force-deletes the branch at the
   consented commit; both together require both permissions and disclose
   both in the result; each result reports `teardown`, `dirty_discarded`,
   `commits_discarded`, `branch_deleted`, and the opaque ID is retired.
4. Drift is refused with nothing removed: `README` changed `A → B` under the
   same `HEAD` and `dirty: true` (`delete_consent_stale`, reason
   `content_changed`); a new untracked file; a new commit (reason
   `head_moved`); a branch switch; consent for `dirty: false` when the
   worktree is dirty; `has_commits_ahead` mismatch; an ignored file
   modified after the preview under the same `HEAD` and `dirty: false`
   (reason `content_changed`); a worktree re-created at the same path
   (identity).
5. Unknown and busy are refused: `inspection: not_inspected` is re-inspected
   by the targeted observation, an unobservable dirty state is
   `worktree_dirty_unknown`, an unobservable ahead state is
   `target_ineligible`, the main worktree is `target_ineligible`, a held
   root guard or base merge in progress is `root_busy`; an unobservable or
   unbounded content observation is `worktree_dirty_unknown` and a nested
   repository is `target_ineligible` with reason `nested_repository`,
   whatever permissions were given; known ignored content without
   `discard_known_dirty` is `worktree_dirty` with reason `ignored_content`;
   a permission the observed facts do not need is accepted but changes
   nothing, and a needed permission that is absent refuses exactly as the
   ordinary path does. Either discard permission without both identities is
   `validation_failed` regardless of `consent.dirty`.
6. Teardown failure is recoverable: an ordinary or consented delete whose
   `.wt/teardown` exits non-zero fails with `internal_error`, retains the
   worktree and its ID, and a subsequent `delete_worktree_confirmed` with
   `skip_teardown: true` and fresh consent succeeds; teardown that modifies
   tracked content after consent refuses `delete_consent_stale` with
   `teardown_ran: true` and the worktree retained.
7. Cancel submits nothing: a preview followed by no command leaves the
   worktree, its ID, the command registry, and the revision unchanged.
8. Compatibility: an older owner serves `delete_worktree` and every other
   command unchanged; a client decodes its capabilities with
   `confirmed_delete` absent as unavailable; `delete_worktree_confirmed`
   submitted to it is `validation_failed` before any effect;
   `delete_requires_teardown` is still published as `true`.

## Explicit Completion Conditions

The real-Git regressions in `design.md` run under `remote_confirmed_delete`
with nonzero passed counts in all three declared gates; a zero-test
selection fails. Every existing `remote_worktree_*` and TUI deletion test
passes unchanged except the worktree capability `operations` assertion.
`docs/guides/USAGE.md` replaces the sentence "Remote clients cannot do any
of this" with the consent contract. Proposal validation is not implementation
evidence; tasks stay unchecked until implemented and verified in a separately
authorized execution.

## Scope / Preserved Contracts

Implementation touches `src/worktree_ops/service.rs` (consent types,
`delete_worktree_with_consent`, content identity oracle in a new
`src/worktree_ops/consent.rs`), `src/vcs/git/commands/status_policy.rs`
(one new read-only status argument set in the inventory),
`src/web/remote_control_api/` (DTOs, preview route, command, result kind,
error code, capability), `src/web/openapi.rs`, focused tests, and
documentation. `delete_worktree`, `create_worktree`, `merge_worktree`, the
TUI confirmations and their key bindings, `DeleteOptions` semantics,
`classify_delete_eligibility`, `classify_delete_drift`, and
`confirm_branch_ref` are unchanged in behavior; the consent path *adds*
checks before and after the existing ones. The `DeleteOptions` doc comments
that say `skip_teardown` is never reachable from `/api/v2` and that only the
TUI may set the discard permissions are rewritten to name the consented
remote path, because they would otherwise be false. No new dependency:
hashing uses `sha2`, file opening uses `std::os::unix` flags. No
Conflux-owned file is written into any worktree, and the oracle excludes no
path.

The tracked `.pre-commit-config.yaml` scopes Rust checks by staged paths; the
declared focused gates stay explicit and no broad build is added as a task.

## Out of Scope / Rollout Handoff

Not included: changing the TUI confirmations to bind content identity
(future work), deleting the main worktree, any stash, backup, commit, or
push before discard, a Web confirmation UI, Server relay code,
`cflx client`/MCP verbs, and any change to `delete_worktree`. The Server
owns the two-step confirmation presentation and must echo the preview's
`consent` object unchanged. Running owners are not restarted by this change.
The `remote-worktree-operations` delta restates the closed command
enumeration in full, so a later proposal touching that requirement must
re-read the canonical spec after this change archives and write its delta
against that text.
