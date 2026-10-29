---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/cli/spec.md
  - openspec/specs/tui-key-hints/spec.md
  - openspec/specs/explicit-target-resume/spec.md
  - src/orchestration/operator_command.rs
  - src/web/remote_control_api/dto.rs
  - src/client/envelope.rs
  - src/cli.rs
verifications:
  - id: add-candidate-proposal-refresh-local
    requirement: One shared operator command previews and applies a narrowly scoped refresh of a candidate worktree's proposal.md from an explicitly named source commit as a hook-retaining Git commit on top of the existing candidate HEAD, with typed refusals, truthful partial-effect reporting, no automatic retry, and content-identity invalidation of stale Acceptance evidence
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/operator_command.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests candidate_proposal_refresh -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests candidate_proposal_refresh -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: add-candidate-proposal-refresh-cli
    requirement: The real cflx client previews and applies a candidate proposal refresh against an isolated owner over the versioned envelope, refuses malformed invocations before contacting the owner, and the OpenAPI contract exposes the command, the preview resource, and the action eligibility
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/client_cli_tests.rs
    evidence: cargo test --test client_cli_tests --features web-monitoring,heavy-tests candidate_proposal_refresh -- --nocapture
    rerun: cargo test --test client_cli_tests --features web-monitoring,heavy-tests candidate_proposal_refresh -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Refresh a candidate proposal without losing applied implementation

**Change Type**: implementation

## Problem

When a proposal is corrected on the base branch after a candidate worktree was already applied, nothing in Conflux can carry the corrected `openspec/changes/<id>/proposal.md` into the candidate. Explicit retry (`retry_change`, TUI F5 on an `error` row) reuses the candidate exactly as it is: resume routing is derived from the candidate's own files and Git state, so the stale proposal — including a stale verification declaration — is what Apply and Acceptance keep reading. The only way out today is manual Git surgery inside a Conflux-managed worktree, which is error-prone and invisible to the owner.

## Supported result

### One explicit, target-scoped operator command

Add one operator command, **refresh proposal**, to the shared process-local operator service, reachable from the typed `/api/v2` surface, from `cflx client refresh-proposal`, and from a TUI action, all through the same transaction. It has two forms that share every rule:

- **Preview** (read-only): given `change_id` and an explicit `source_ref`, resolve and report what an apply *would* do. It creates no command record, submits no intent, and mutates nothing.
- **Apply**: given `change_id`, `source_ref`, `expected_source`, and `expected_head` (both full 40-hex commit IDs taken from a preview), perform exactly one narrowly scoped Git commit in the candidate worktree.

### Source and path (frozen)

- `source_ref` is required in every form; there is no default and no hardcoded `main`/`develop`. It is resolved in the project repository with `git rev-parse --verify --end-of-options <source_ref>^{commit}`; failure is the typed refusal `source_ref_unresolvable`. The TUI supplies the run's configured base branch name as `source_ref` and shows it in the confirmation; the CLI requires `--source-ref`.
- The only content read from the source commit is the blob at exactly `openspec/changes/<change_id>/proposal.md`. Absence is `source_proposal_missing`. No other proposal artifact (`design.md`, `tasks.md`, `specs/`), no implementation file, no arbitrary path, and no merge or rebase of the base branch is involved.
- The candidate path is the same repository-relative path at the candidate's current `HEAD`; absence there is `candidate_proposal_missing`.
- The source branch and every other worktree are read-only to this command.
- Both source/candidate tree entries and the on-disk candidate leaf MUST be regular non-symlink files, and every candidate path component must resolve inside the managed worktree without traversing symlinks. Otherwise refuse `unsafe_proposal_path` before any write. Preserve the candidate file mode; never follow or create a symlink. Ref and path arguments use option-terminating Git forms and literal pathspecs, never shell interpolation.

### Target eligibility (frozen; published as `actions.refresh_proposal`)

Eligibility is a pure function of the owner's authoritative facts, like `classify_force_stop_change`: the target MUST be tracked, MUST have a registered managed worktree, MUST own no live managed process, and its display status MUST be one of `error`, `stopped`, `stalled`, `blocked` with blocker kind `external`, or `not queued`. Every other row is refused with a stable reason: `unknown_target`, `no_candidate_worktree`, `terminal_target` (`archived`, `merged`, `pushed`, `rejected`), `target_active` (`preparing`, `applying`, `accepting`, `rejecting`, `archiving`, `resolving`, or any row owning a live managed process), `target_admitted` (`queued`, or `blocked` on a dependency), `merge_wait`, `resolve_wait`, `reject_wait`. Eligibility is independent of process mode (`Select`, `Running`, `Stopping`, `Stopped`, `Error`) because the command touches no scheduler state.

### Command-time preconditions (frozen; evaluated inside the shared transaction)

In order: eligibility is re-validated at the admitted revision; `source_ref` is resolved and MUST equal `expected_source`; candidate `HEAD` MUST equal `expected_head` (either mismatch is `refresh_expectation_mismatch`, reporting the current values); the candidate MUST be clean under `git status --porcelain --untracked-files=all` with no Conflux-owned path excluded (`candidate_dirty`, listing entries); the candidate's Git directory MUST hold no in-progress operation (`MERGE_HEAD`, `CHERRY_PICK_HEAD`, `REVERT_HEAD`, `REBASE_HEAD`, `rebase-merge/`, `rebase-apply/`, `BISECT_LOG` → `candidate_operation_in_progress`); then the source and candidate blobs are read. Identical blobs settle as the success `unchanged` with no write.

### The write (frozen)

1. Write the source blob's bytes to the candidate path.
2. `git add -- openspec/changes/<change_id>/proposal.md` (pathspec-limited; never `-A`).
3. `git commit -m "Refresh proposal: <change_id> from <source_commit first 12 hex>"` in the candidate worktree **with repository hooks enabled** (never `--no-verify`). Hook stdout and stderr stream to the owner log with structured change/operation context and a bounded tail is retained in the settled result.
4. Post-commit verification: `HEAD` advanced by exactly one commit whose sole parent is `expected_head`; `git diff-tree --no-commit-id --name-only -r HEAD` lists exactly the path; the worktree is clean afterwards.

Effects semantics: `file_written` records whether the command wrote its target, `staged` records whether its staging step completed (not whether changes remain in the index after commit), `committed` records observed HEAD advancement, and `clean_after` is the actual final Git cleanliness. Any hook can change files/index/HEAD, so re-read Git even on failure and report actual effects, never assume committed=false merely from a nonzero exit.

Outcomes: success `refreshed` when step 4 holds. A hook exiting non-zero is `hook_rejected`: the written file stays written and staged (recoverable), nothing is reset or cleaned, and the result says `file_written: true, staged: true, committed: false`. A Git failure at commit is `commit_failed` with the same retained state. A commit whose tree change is not exactly the path (a hook that re-staged other files) is `refresh_commit_unexpected_tree`, and a commit that left the worktree dirty is `hook_left_workspace_dirty`; in both cases `committed: true` is reported, `HEAD` is left where the commit put it, and no broad cleanup, reset, or amend is attempted. Every result carries `source_commit`, `source_blob`, `candidate_head_before`, `candidate_head_after`, `candidate_blob_before`, `path`, and `effects {file_written, staged, committed, clean_after}` so a caller reads what actually happened rather than inferring it.

### What the command never does

It never starts, retries, queues, marks, unmarks, dequeues, stops, resolves, archives, or merges; the row's display status, queue intent, execution mark, blocker, terminal evidence, attention, and retry eligibility are unchanged by a refresh, and unrelated changes are untouched. It never writes a Conflux-owned file into the candidate (the commit is the deliverable), never persists refresh history outside the repository as workflow authority, and adds no configuration or policy toggle. It is not exposed over `cflx client mcp` in this change: the closed `cflx_control` action list is unchanged.

### Why ordinary retry then reads the corrected candidate

Resume routing is recomputed from candidate files and Git on the next explicit retry, exactly as today; the Acceptance manifest's `change_input_digest` reads the candidate's `proposal.md`, and evidence records are bound to the candidate commit and tree. A refresh therefore changes the manifest fingerprint, makes every prior gate evidence record decide `Rerun`, and makes any in-memory unchanged-input hold inapplicable — by content identity, without any new state. This change proves those properties with tests; it does not add a second mechanism.

### API, CLI, TUI (frozen shapes)

- **API.** `GET /api/v2/proposals/{change_id}/refresh-preview?source_ref=<ref>` returns the preview (`instance_id`, observed `state_revision`, `eligible`, `refusal` with the stable reason when any precondition fails, `source_ref`, `source_commit`, `candidate_head`, `path`, `source_blob`, `candidate_blob`, `no_op`, unified `patch` from candidate to source) on both listeners. `POST /api/v2/commands` accepts type `refresh_proposal` with `change_id`, `source_ref`, `expected_source`, `expected_head`, ordinary `expected_revision` fencing and idempotency identity; `deny_unknown_fields` applies; the settled result kind is `refresh_proposal` carrying the fields above and `outcome` (`refreshed` | `unchanged`). Each `ChangeResource` publishes `actions.refresh_proposal` with the lifecycle reasons above. Capabilities advertise the command and the preview resource; the generated OpenAPI document includes all of them.
- **CLI.** `cflx client refresh-proposal <change-id> --source-ref REF --dry-run [--json]` → operation `refresh_proposal_preview`, success outcome `previewed` (exit `0`). `cflx client refresh-proposal <change-id> --source-ref REF --expected-source COMMIT --expected-head COMMIT [--json]` → operation `control_refresh_proposal`, success outcomes `refreshed` and `unchanged` (exit `0`). Missing `--source-ref`, `--dry-run` combined with an expectation, an apply without both expectations, or a non-40-hex expectation is a `usage_error` before any owner contact. Lifecycle refusals map to `target_ineligible`; command-time refusals map to the new outcome `refresh_refused` (exit `28`) with `detail.reason` and `detail.effects`; `change_not_found`, `revision_conflict`, `owner_not_command_capable`, `owner_restarted`, and transport outcomes keep their meanings. `--project-dir`/`--unix-socket` routing is unchanged.
- **TUI.** On an eligible Changes row, `p` opens a confirmation popup that runs the same preview (source ref = the run's configured base branch) and shows source ref, source commit, candidate head, blob identities, added/removed line counts, or the no-op notice; `Enter`/`y` submits the apply with the previewed expectations through the shared transaction; `Esc` cancels. The key hint `p: refresh proposal` is shown only when `actions.refresh_proposal` is allowed for the focused row. An ineligible row ignores `p`.

## Change boundary

- `src/orchestration/operator_command.rs` and `operator_coordinator.rs` (command, eligibility classifier, preview, Git steps through the existing worktree Git backend), `src/web/remote_control_api/{dto,executor,projection}.rs` and router (command, preview route, actions, OpenAPI), `src/client/{envelope,control}.rs` and `src/cli.rs` (verb, operations, outcomes), `src/tui/{key_handlers,state,render}.rs` (key, popup, hint), `AGENTS.md`/docs for the client verb.
- Spec deltas: ADDED requirements in `operator-command-execution`, `remote-control-api`, `cli`, and `tui-key-hints`.
- Out of scope: MCP exposure, refreshing other artifacts, automatic retry, upstream fetch, any change to Apply/Acceptance/retry semantics.

## Acceptance

1. **Real Git end to end.** On a disposable repository with a base branch, a candidate worktree holding an applied implementation commit, and a corrected `proposal.md` committed on the base branch: preview reports the frozen facts and a non-empty patch; apply creates exactly one commit on top of the previous candidate `HEAD` whose only tree change is the path; the previous `HEAD` is an ancestor of the new one; the base branch tip, the candidate's implementation files, and every other candidate file are byte-identical before and after; a second apply with fresh expectations settles `unchanged`; a subsequent explicit retry's Acceptance manifest `change_input_digest` and fingerprint differ from the pre-refresh values, a pre-refresh gate evidence record decides `Rerun` on `commit_oid`, and the candidate file read by the Apply/Acceptance prompt builder contains the corrected text.
2. **Refusals and partial effects.** Each of: wrong `expected_source`, wrong `expected_head`, stale `expected_revision`, replaced owner incarnation, dirty candidate (staged, unstaged, and untracked variants), in-progress merge, active or admitted target, terminal target, merge-wait target, missing worktree, missing source blob, missing candidate blob, unresolvable ref, a real `pre-commit` hook exiting non-zero, and a hook that re-stages another file — produces the frozen typed reason, preserves marks, queue intent, mode, hook configuration, scheduler state and unrelated targets, and reports actual file/index/HEAD effects truthfully. Pre-write refusals preserve HEAD/index exactly; hook rejection retains the state the hook actually left, while an unexpected-tree hook may have already committed and MUST be reported as such rather than promising unchanged HEAD. Effects are observed facts, never assumed from the attempted step.
3. **One service, three fronts.** Production API router tests and TUI key-handler tests prove the API command, the CLI, and `p` submit the identical operator command and receive identical settlement; the TUI popup shows the preview facts and submits nothing on `Esc` or on an ineligible row; MCP `cflx_control` still refuses an unknown action. `tests/client_cli_tests.rs` drives the isolated owner fixture with a real candidate repository through `cflx client refresh-proposal --dry-run --json` and the apply form, asserts one JSON envelope per invocation, the frozen operations/outcomes/exit codes, usage errors before owner contact, and that `cflx openapi` lists `refresh_proposal`, the preview route, and `actions.refresh_proposal`.
4. **Gating.** Every new test name contains `candidate_proposal_refresh`; both declared commands select at least one test (zero selected is a failing gate); tests over one second are `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`; disposable repositories, hooks, and state roots live under temporary directories outside any Conflux-managed worktree, and no test touches a live owner, the original incident candidate, or pushes anything.
5. **Spec and docs coherence.** The four ADDED requirements archive cleanly without altering existing requirements; `AGENTS.md` and the client help document the verb. Strict validation of this change is not implementation evidence.

## Delivery boundary

Do not push, release, replace a live owner, edit global or project configuration, or touch the original scope-drill incident candidate. Work only in the Conflux-managed candidate worktree of this independent develop-based repository; keep `main` untouched. Do not mark any task complete from prose, from a zero-test gate, or from an authoring-time strict validation pass.
