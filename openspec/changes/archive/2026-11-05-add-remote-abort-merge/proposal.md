---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/remote-worktree-operations/spec.md
  - openspec/specs/tui-worktree-merge/spec.md
  - docs/guides/USAGE.md
verifications:
  - id: remote-merge-abort-vcs
    requirement: The abort-on-conflict Git merge reports the real result of git merge --abort and verifies restoration from repository evidence instead of assuming it
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/vcs/git/commands/merge.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-merge-abort-api
    requirement: Explicit opt-in abort-on-conflict merge command refuses predicted conflicts, aborts real ones with typed restoration evidence, preserves merge_worktree defaults and on_merged behavior, and allows a following merge
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/web/remote_control_api/tests/mod.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests remote_merge_abort -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: remote-merge-abort-contract
    requirement: Generated OpenAPI, capabilities, error codes, and result kind publish the abort surface without redefining merge_conflict_preserves_state, and older owners decode as unavailable
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/openapi_contract_tests.rs
    evidence: cargo test --features web-monitoring --test openapi_contract_tests remote_merge_abort -- --nocapture
    rerun: cargo test --features web-monitoring --test openapi_contract_tests remote_merge_abort -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Add remote abort-on-conflict worktree merge

**Change Type**: implementation

## Problem / Context

Conflux-server (CFSRV-55, OP-28) needs the TUI's `M` behavior from the Web:
merge a worktree branch into base and, if it conflicts, abort and leave the
base repository where it was. At develop `62d256107` the TUI submits
`ConflictPolicy::AbortOnConflict` (`src/tui/command_handlers.rs:770-772`)
while `/api/v2` `merge_worktree` is fixed to `PreserveConflict`
(`src/web/remote_control_api/worktrees.rs:591-608`), the capability states
`merge_conflict_preserves_state: true` and
`merge_conflict_recovery: local_or_tui_required`, and the spec forbids remote
abort (`openspec/specs/remote-worktree-operations/spec.md`, "Shared
conflict-preserving worktree merge"). Existing remote clients may depend on
the intermediate state being left for local resolution, so the default must
not change.

The abort path itself is not yet trustworthy. `commands::merge_branch`
discards the result of `git merge --abort` and then returns an error text
that says the merge was aborted (`src/vcs/git/commands/merge.rs:375-383`);
the backend maps it to an opaque `Internal` error (`src/worktree_ops/git_backend.rs:174-179`),
so the TUI and the spec scenario "Merge aborted" can be stated when the base
is in fact still holding `MERGE_HEAD`. `classify_merge_eligibility` already
refuses a *predicted* conflict only under `AbortOnConflict`
(`src/worktree_ops/service.rs:700-706`).

## Proposed Solution

Add an explicit, separately advertised command and make the abort path tell
the truth:

- `merge_worktree_abort_on_conflict` (target opaque `worktree_id`, empty
  params) runs the shared merge with `AbortOnConflict`. `merge_worktree`
  keeps `PreserveConflict`, its params, errors, capability sentence, and
  behavior byte-for-byte, so every existing client sees no change.
- A predicted conflict (the fresh targeted inspection reports conflict
  files) is refused before any Git mutation as `merge_conflict_predicted`
  with the files in the typed result.
- A real conflict runs `git merge --abort`, records its exit status, and then
  verifies restoration from the repository: `HEAD` equals the pre-merge
  commit, no `MERGE_HEAD` exists, and the root working directory is clean
  under the read-only status policy. The result reports `restored` as
  `confirmed`, `failed`, or `unknown`, never from the attempt alone. A
  confirmed restoration settles `merge_conflict_aborted` (the merge failed;
  the root is proven clean at the pre-merge `HEAD`; a next merge is
  possible). Anything else settles `merge_abort_failed` with the observed
  facts, the root reports `root_busy` for incompatible mutations while `MERGE_HEAD`
  remains; if it is absent but the root is dirty, the ordinary clean-state
  check still refuses a merge until local or TUI recovery. Nothing claims
  the state was preserved on purpose.
- The same typed abort report flows to the TUI: `merge_branch` becomes a
  wrapper over the new typed function and its error text names the verified
  outcome, and the `MergeFailed` event carries it. `on_merged` runs only
  after a completed merge, exactly as today.
- Capabilities gain `worktrees.merge_abort`; `merge_conflict_preserves_state`
  and `merge_conflict_recovery` keep their values with descriptions narrowed
  to `merge_worktree`. The worktree `operations` list gains `merge_abort`.
  The Server never runs `git merge --abort` itself.

## Acceptance Criteria

1. `merge_worktree` is unchanged: `PreserveConflict`, `merge_conflict` with
   files and `local_or_tui_required`, retained intermediate state, `root_busy`
   afterwards, and every existing `remote_worktree_*` merge test passes
   unmodified.
2. `merge_worktree_abort_on_conflict` on a cleanly mergeable worktree merges
   with `--no-ff`, runs `on_merged` exactly once, publishes
   `MergeStarted`/`MergeCompleted`, and settles `succeeded` with result
   `outcome: merged` and no abort report.
3. A predicted conflict is refused `merge_conflict_predicted` with the
   conflict files, `MERGE_HEAD` never appears, base `HEAD` is unchanged, and
   `MergeFailed` is published with the predicted-conflict text.
4. A real conflict (service and VCS levels) performs the abort and reports
   `premerge_head`, `conflict_files`, `abort.exit_ok`, `head_after`,
   `merge_head_present`, `clean_after`, and `restored`; `confirmed` is
   reported only when all three verifications pass and settles
   `merge_conflict_aborted`; a failing `git merge --abort` (injected through a
   PATH-shimmed `git`) reports `exit_ok: false`, `restored: failed`, settles
   `merge_abort_failed`, and a subsequent incompatible mutation is `root_busy`;
   an unreadable verification reports `restored: unknown` and the same
   failure code.
5. After a confirmed abort the next merge of another clean branch succeeds
   through either merge command; `on_merged` never ran for the aborted one.
6. The TUI `M` path receives the same typed report: on confirmed restoration
   the log and popup say the merge was aborted and the base restored; on
   failure they say the abort failed or restoration is unconfirmed and that
   local recovery is required; the legacy sentence "Merge aborted" is never
   emitted when restoration was not confirmed.
7. Compatibility: an older owner serves `merge_worktree` and every other
   command unchanged; a client decodes its capabilities with `merge_abort`
   absent as unavailable; the new command submitted to it is
   `validation_failed` before any effect; `merge_conflict_preserves_state`
   remains `true`.

## Explicit Completion Conditions

The regressions in `design.md` run under `remote_merge_abort` with nonzero
passed counts in all three declared gates; a zero-test selection fails. The
VCS gate includes a real Git conflict with a real `git merge --abort` and an
injected abort failure. `docs/guides/USAGE.md` documents both commands and
the restoration evidence. Proposal validation is not implementation
evidence; tasks stay unchecked until implemented and verified in a
separately authorized execution.

## Scope / Preserved Contracts

Implementation touches `src/vcs/git/commands/merge.rs` (typed aborting merge
and the `merge_branch` wrapper), `src/worktree_ops/git_backend.rs` and
`service.rs` (`MergeAttempt::ConflictAborted`, typed errors, event text),
`src/tui/command_handlers.rs` (message mapping only),
`src/web/remote_control_api/` (command, result kind, error codes, capability,
port method), `src/web/openapi.rs`, focused tests, and documentation.
`merge_branch_preserving_conflict`, `merge_worktree`, `resolve_merge`, the
`on_merged` hook contract, conflict prediction, the root guard, and
authentication are unchanged. No new dependency. No file is written into
any worktree.

The tracked `.pre-commit-config.yaml` scopes Rust checks by staged paths; the
declared focused gates stay explicit and no broad build is added as a task.

## Out of Scope / Rollout Handoff

Not included: changing the `merge_worktree` default, a remote conflict
resolution editor, remote `resolve`, any Server-side Git compensation, a
`conflict_policy` parameter on the existing command, `cflx client`/MCP verbs,
and changes to `resolve_merge`. Non-conflict merge acceptance proceeds
independently on the existing command. Running owners are not restarted by
this change.
