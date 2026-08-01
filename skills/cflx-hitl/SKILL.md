---
name: cflx-hitl
description: "Handle every human-intervention boundary in a Conflux automated loop. Use whenever a Conflux user must decide, approve, provide credentials or external evidence, resolve policy or schema ambiguity, choose rejection disposition, authorize merge, push, deployment, publication, or process control, handle a stalled acceptance hold, or resume automation afterward. This is the general operator-facing HITL skill for Conflux, not a troubleshooting-only procedure. Preserve the active frontend and orchestration mode unless the user explicitly chooses otherwise."
---

# Conflux Human-in-the-Loop Operations

Use this skill for every point where Conflux automation legitimately needs human intent, authority, secret material, external action, or non-repository observation.

## Operating principle

Conflux agents perform repository-verifiable work autonomously. Humans own intent, authority, secrets, external systems, risk acceptance, and process control.

Do not manufacture HITL for work Conflux can complete from repository evidence. Do not let automation silently decide matters that change product intent, security policy, external state, irreversible outcomes, or user-owned processes.

## Protect the active orchestration session

Before mutating process or runtime state, locate and read the repository's orchestration-owner metadata. Use Conflux status or frontend information when available; otherwise discover the owner metadata from the repository's VCS metadata directory rather than assuming a fixed path.

Record the owner process identifier, frontend or mode, workspace, start time, and endpoint when present.

- Treat the owner process as user-owned.
- Do not signal, stop, restart, or replace it without explicit approval.
- Do not replace an interactive TUI or server session with a non-interactive run.
- Do not start a second orchestrator while a live owner exists.
- Preserve the same frontend and mode on resume unless the user explicitly requests another mode.
- Read-only inspection does not require approval.
- Use platform-native process inspection and termination facilities. Do not assume POSIX signals, `ps`, `kill`, `/proc`, a shell, or a particular operating system.

## Human intervention map

### 1. Intent and proposal approval

Human input is required when repository context cannot establish:

- the user outcome or problem to solve
- scope and explicit non-goals
- a product, UX, policy, security, compatibility, or migration trade-off
- whether independent work must ship together
- acceptance of irreversible or externally visible behavior

Infer routine implementation details from the repository. Ask only for unresolved intent, and present the proposal for approval before orchestration starts.

Persist the decision in proposal, design, task, constitution, or specification artifacts. Conversation-only intent is not durable loop input.

### 2. Permission to start or control orchestration

Human authority is required to:

- start a requested run when none is active
- stop or cancel an active run
- restart or change the frontend or orchestration mode
- discard or recreate a worktree or workspace
- clear runtime-owned state
- change targets, parallelism, remote integration, or execution policy when that alters the active operation

Permission to implement does not grant permission to stop, replace, merge, push, deploy, publish, or delete runtime state.

### 3. Credentials and secret material

Blocker category: `credential`.

Human or authorized secret-management input is required when a non-mockable operation needs a missing credential, signing identity, token, certificate, account, or device authorization.

- Never place secret values in public artifacts, logs, task text, command arguments visible to other processes, or ordinary conversation when a secure channel exists.
- Prefer fixture or local verification for change-blocking acceptance.
- Keep credentialed external checks as operational observations unless genuinely non-mockable.
- Record only the credential identifier, owner, secure provisioning method, and rerun action.

### 4. External approval

Blocker category: `external_approval`.

Use for authority held outside the repository loop, such as legal, compliance, store review, customer sign-off, security review, or release approval.

Record the approver role, requested decision, evidence location, expiry when relevant, and the exact resume action.

### 5. Policy or risk decision

Blocker category: `policy`.

Human input is required when continuing would amend or choose between security, privacy, retention, financial, compatibility, release, or operational policies.

Automation may present evidence and recommend an option. It must not silently weaken a policy to make acceptance pass. Persist the chosen policy in the authoritative specification or constitution before resuming.

### 6. External service or infrastructure action

Blocker categories: `external_service` or `infrastructure`.

Use when work requires an unavailable service, network or naming change, package registry, cloud resource, device, daemon, deployment environment, or operator-controlled infrastructure.

Separate:

- repository-fixable configuration or code, which returns to apply
- operator-owned environment action, which enters HITL
- transient failures, which remain resumable stalled holds rather than rejection

State the exact external action, owner, portable verification procedure, and safe retry point.

### 7. Pending non-local verification

Blocker category: `pending_verification`.

Use when a correctly wired job, physical-device check, deployed smoke test, benchmark, or external observation has started or must run outside repository-local acceptance.

Do not claim success before evidence exists. Record the trigger, evidence location, expected completion condition, timeout or escalation owner, and rerun action.

Post-integration, physical-device, deployed-service, external-approval, and credentialed checks are normally operational observations, not repository-local archive or merge blockers.

### 8. Schema or upstream contract incompatibility

Blocker category: `schema_incompatibility`.

Human input is required when two authoritative contracts cannot both be satisfied and repository evidence cannot establish which authority should change.

Present both contract clauses, compatibility consequences, migration options, and a recommendation. Persist the selected authority and migration in specifications before resuming.

Do not misclassify an ordinary parser defect or incomplete adapter as HITL.

### 9. Product or architectural decision

Blocker category: `human_decision`.

Use only when multiple valid paths remain and choosing one changes product intent, public behavior, architecture ownership, migration guarantees, or irreversible cost.

Do not use `human_decision` for missing investigation, vague tasks, failed tests, dirty workspaces, or work an apply agent can perform.

### 10. Rejection disposition

When rejection review proposes a terminal outcome, present these human-facing choices:

- `CONFIRM`: the intent is invalid, obsolete, contradictory, or intentionally abandoned
- `BLOCK`: the intent remains valid but must wait on a real prerequisite
- `RESUME`: evidence is insufficient or repository-only work can resolve it

Explain evidence and consequences. A rejection artifact is a review proposal, not automatic authorization to abandon the change.

### 11. Merge conflict or cumulative-base choice

Routine textual conflicts with a uniquely correct preservation of both accepted intents may be resolved autonomously.

Escalate when resolution requires choosing which accepted behavior or specification wins, dropping functionality, changing public contracts, rewriting shared history, force-updating a remote, or accepting failed verification.

Present the conflicting intents and a recommendation before asking the user to decide.

### 12. Merge, push, deployment, publication, and release

Human authority is required unless the original instruction explicitly included the exact action and target.

Treat these as separate permissions:

- create a commit
- merge into the base branch
- push to a named remote
- create or update a pull request
- deploy to a named environment
- publish a package or release
- communicate externally

One permission does not imply another. Preserve repository-specific release gates. Never substitute fixture evidence for deployed, live-service, external, or physical-device evidence.

### 13. Final operational acceptance

After repository work merges or archives, HITL may still be needed to accept a release, production behavior, physical-device result, customer outcome, or policy exception.

Report repository completion separately from operational acceptance. Keep unresolved observations visible without reopening or falsifying repository-local completion.

## Stalled hold protocol

Only a blocker that remains true under an identical acceptance rerun may enter durable stalled state. The reviewer must declare `deterministic: true`; missing or false determinism stays on the bounded retry path and is never persisted.

Host load, timing-budget breaches, transient network or service failures, scheduler contention, and other runtime observations are non-deterministic unless they identify a stable external prerequisite independent of the observation. Retry them without creating durable state.

A deterministic structured acceptance blocker uses exactly one supported category:

- `credential`
- `external_approval`
- `policy`
- `external_service`
- `pending_verification`
- `infrastructure`
- `schema_incompatibility`
- `human_decision`

Conflux persists accepted stalled holds outside the change worktree. Locate the state through Conflux status/log output or runtime configuration. Do not assume a home directory, XDG path, drive letter, path separator, shell expansion rule, or operating system.

A persisted stall record binds repository identity, change identity, workspace identity, apply revision, category, evidence, next action, and resumability. It can survive process restart.

A queue summary such as `blocked_only_no_dispatchable_candidates` is not the root cause. Inspect the detailed queue classification and the persisted stall record before making a claim.

## HITL request format

Ask one bounded question that a human can decide:

```text
Context: <change, phase, active frontend/mode>
Why automation stopped: <specific authority or missing external input>
Evidence: <repository artifact, log event, runtime-state record, or external evidence reference>
Decision needed: <one concrete decision>
Recommendation: <preferred option and reason>
Options and consequences:
A. <option>
B. <option>
Resume condition: <durable artifact/state and exact next route>
```

Never ask “What should I do?” without options and consequences.

## Record the answer durably

A human response is input, not proof. Validate it and persist it in the appropriate authority:

- product or scope: proposal or design
- behavior or policy: canonical specification or constitution
- implementation recovery: tasks, source, and verification
- external approval: referenced approval evidence
- credential: secure store plus a non-secret reference
- operational observation: tracked evidence location
- process permission: current session record, limited to the named action

Only then resume automation.

## Clear a resolved stalled hold

Clear runtime state only after the blocker is resolved or proven invalid and the user explicitly approves the mutation.

1. Locate and read owner metadata; preserve the active frontend and mode.
2. Ask the user to stop the active frontend, or obtain explicit permission to stop it with platform-native facilities.
3. Confirm the owner process exited and ownership was released.
4. Locate and re-read the exact stall record through Conflux output or runtime configuration.
5. Verify repository, change, workspace, and apply-revision identity.
6. Remove only that record using an official Conflux operation when available; otherwise use a platform-appropriate filesystem operation against the exact confirmed record. Never remove the state collection or unrelated records.
7. Resume the same frontend and mode unless the user requested otherwise.
8. Verify fresh logs show the expected analysis, acceptance, archive, or merge route.

A newer workspace commit does not automatically invalidate a stall tied to an earlier apply revision.

## Cases that are not HITL

Return to autonomous work for:

- repository-fixable compile, lint, test, or validation failures
- missing implementation or incomplete tests
- dirty state created by the agent
- unambiguous task progress updates
- routine dependency ordering
- deterministic conflict resolution preserving both intents
- retries within the configured budget
- questions answerable from source, specifications, configuration, logs, or current runtime state

## Completion report

```text
HITL boundary: <type/category>
Decision or authority: <who supplied what>
Durable record: <artifact, state, or evidence reference>
Automation resumed as: <same frontend/mode and route, or intentionally stopped>
Verification: <fresh evidence>
Remaining operational observations: <none or explicit list>
```
