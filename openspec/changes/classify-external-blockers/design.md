# Design: External blocker lifecycle classification

## Decision ownership

Agents observe and report facts; Conflux decides lifecycle state. Apply and Acceptance outputs are untrusted protocol input until parsed and validated. This keeps UI and scheduling semantics deterministic across agent runtimes and prevents an agent from assigning canonical status by choosing a verdict word.

## Classification model

Use two independent concepts:

- lifecycle status: `Blocked` or `Stalled`;
- blocker kind: dependency or external, with source/category metadata for external blockers.

`Blocked` means useful execution is currently ineligible because a named prerequisite has not changed. `Stalled` means Conflux stopped automatic execution after no semantic progress, repeated findings, or exhausted retry/repair policy. Dependency waits remain derived from the project dependency graph. External waits require validated structured evidence and do not become dependency edges.

A compatibility verdict token is transport syntax, not classification evidence. `gated` and legacy `blocked` remain parseable, but only a complete structured payload can become external `Blocked`.

## Required external blocker fields

The classifier requires:

- origin phase (`apply` or `acceptance`);
- supported category;
- concrete non-empty evidence;
- the external prerequisite or owner;
- a verifiable unblock condition;
- next action;
- resumability.

Repository-fixable work and mockable dependencies are not external blockers. Missing or contradictory fields produce protocol correction or a repository-fixable finding, not an inferred external wait.

## State and restart boundary

Reducer-owned classification is in memory and may suppress dispatch and drive display during the process lifetime. It cannot establish completion, acceptance pass, archive readiness, merge eligibility, or integration. No durable state is added outside the workspace.

On restart, Conflux reconstructs routing from workspace files, git state, and base comparison. Repository-visible Implementation Blocker evidence may be re-evaluated, but previous in-memory classification has no authority. Acceptance for a complete unarchived Apply revision runs again rather than inferring pass or preserving a hidden hold.

## Scheduling and retry

External-blocked changes are excluded from ordinary dispatch. They do not prevent unrelated ready changes from progressing and do not masquerade as proposal dependencies. Explicit retry first reconciles current workspace identity and evidence. It clears or changes classification only when the current condition supports retry; unsupported or unchanged blockers retain their evidence.

## Surface contract

Reducer status is authoritative for TUI, WebSocket/API, and dashboard. Payloads carry lifecycle status plus blocker kind and detail. Surfaces may summarize detail but must not independently infer blocked versus stalled from filenames, prose, or task text.

## Compatibility

Legacy acceptance tokens remain accepted during migration. Existing dependency-blocked display remains `blocked`. Existing stalled rows retain retry behavior unless an external blocker payload validates under the new classifier. No source or UI consumer may use token spelling as the lifecycle decision.
