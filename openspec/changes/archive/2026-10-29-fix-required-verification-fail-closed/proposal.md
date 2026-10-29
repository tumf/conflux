---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/proposal-metadata/spec.md
  - openspec/specs/cflx-proposal-validation/spec.md
  - docs/guides/VERIFICATION_EVIDENCE.md
  - src/orchestration/acceptance/execution_manifest.rs
  - src/orchestration/acceptance/verification_evidence.rs
  - src/openspec_cmd/validation.rs
  - src/openspec_cmd/verify.rs
verifications:
  - id: fix-required-verification-fail-closed-local
    requirement: Required repository-local change-blocking declarations that cannot be supervised are typed declaration defects in the shared planner, strict validation, verify --plan, and the native Acceptance boundary, and never silently leave the manifest
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/acceptance/execution_manifest.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests required_verification_fail_closed -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests required_verification_fail_closed -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: fix-required-verification-fail-closed-cli
    requirement: The real cflx binary reports declaration defects with a nonzero exit status from openspec validate --strict, openspec validate --archive-gate, and openspec verify --plan on a disposable repository, and launches no declared command for an invalid required manifest
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: cargo test --test run_exit_tests --features web-monitoring,heavy-tests required_verification_fail_closed -- --nocapture
    rerun: cargo test --test run_exit_tests --features web-monitoring,heavy-tests required_verification_fail_closed -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fail closed on invalid required verification declarations

**Change Type**: implementation

## Problem

A `completion_role: change-blocking`, `execution_class: repository-local`, `phase: pre-integration` verification declaration is the proposal's statement that Acceptance must run that command before any verdict. Today that statement can silently vanish:

- `eligible_gates` in `src/orchestration/acceptance/execution_manifest.rs` selects gates through `is_eligible_declaration`, which maps *every* `eligible_request` error — `declaration_shell_syntax`, `declaration_missing_command`, `declaration_missing_automation`, `declaration_unstorable_id`, `declaration_missing_id` — to "not eligible". A required declaration whose command contains a shell operator, whose quotes do not tokenize, or whose automation path is missing is dropped from the manifest without a trace; the manifest becomes a *valid empty allowlist*, `classify_gate_results` returns `Proceed`, and the semantic reviewer is launched with the statement that no blocking gate is declared.
- After selection, `eligible_gates` sorts by verification ID and `dedup_by` ID. Two declarations with the same ID keep the first in authoring order; a second declaration with a *different* command is hidden rather than reported.
- `cflx openspec validate --strict` rejects duplicate IDs and missing fields but never evaluates whether a change-blocking command is supervisable, so the author learns nothing before Apply. `cflx openspec verify --plan` reports an unsupervisable declaration as a `rerun` decision with reason `declaration_shell_syntax` and exits `0`.

This was reproduced in isolation on installed v0.6.327. A whole-Acceptance false PASS has *not* been demonstrated — the reviewer still performs semantic review — but the runtime-owned blocking boundary that `docs/guides/VERIFICATION_EVIDENCE.md` promises is not closed, which contradicts constitutional law 3 (truthful completion).

## Supported result

### Vocabulary (frozen)

- **Required declaration**: a verification declaration whose structured metadata says `completion_role: change-blocking`. Required-ness is read from that explicit field only. Legacy declarations without role metadata and `completion_role: operational-observation` declarations are *non-required*; they are never gates, and their commands are never evaluated for supervisability by this contract.
- **Declaration defect**: a typed, repository-local defect of one proposal's declaration list. A defect names the verification ID (or the 1-based list position `<position N>` when the ID is missing), the offending field, a stable code, and a human-readable detail.
- **Invalid required manifest**: the planning outcome when at least one declaration defect exists. It is distinct from a *legitimately empty allowlist* (zero required declarations, zero defects).

### Shared declaration planner

One production function (the *declaration planner*; its Rust name is not normative) replaces `eligible_gates` as the only way to derive the Acceptance gate set from a proposal's ordered declaration list. It returns both the ordered eligible gate requests and the ordered list of declaration defects. Its three consumers are `cflx openspec validate` (strict and archive-gate modes), `cflx openspec verify` (plan and run modes), and the native Acceptance manifest builder. No consumer may re-implement selection by filtering `eligible_request` failures away.

Planner rules, in order:

1. **Duplicates first.** If the same non-empty trimmed `id` appears more than once in the declaration list — with identical content or with different content — every occurrence is a defect `declaration_duplicate_id` on field `id`, naming the ID and all positions. No gate is produced for a duplicated ID. There is no deduplication step.
2. **Required-ness.** Each declaration is classified required or non-required from `completion_role` alone. Non-required declarations produce neither gates nor defects.
3. **Role consistency of required declarations.** A required declaration whose `phase` is not `pre-integration` is a defect `declaration_role_conflict` on field `phase`; one whose `execution_class` is not `repository-local` is the same code on field `execution_class`. These are the combinations strict validation already refuses; the planner reports them so Acceptance never silently treats them as non-required.
4. **Identity.** A required declaration with an absent or empty `id` is `declaration_missing_id`; an ID that is not a safe single path component is `declaration_unstorable_id`.
5. **Command.** The command is `rerun`, or `evidence` when `rerun` is absent, trimmed. An empty command is `declaration_missing_command` on field `rerun`. A command containing any of the shell metacharacters `|`, `&`, `;`, `<`, `>`, `(`, `)`, `` ` ``, `$`, or one the existing POSIX-quote tokenizer cannot split (unbalanced quotes), or one that tokenizes to an empty argv or to a blank argument, is `declaration_shell_syntax` on field `rerun` (or `evidence` when that was the source). The detail names the offending construct — the first metacharacter and its character offset, or `unbalanced quote` — and does **not** echo the whole command text.
6. **Automation.** An absent, empty, absolute, or parent-traversing `automation` path is `declaration_missing_automation`.
7. **Result.** With zero defects, the gates are the required declarations' supervisable requests in canonical verification-ID order. With one or more defects, the planner still returns the complete defect list but the gate list MUST be treated as unusable by every consumer: an invalid required manifest offers no gate to run and no gate to reuse.

The planner evaluates no PATH lookup, no Git state, and no network. Whether a declared executable exists is a separate, later question (`tool_unavailable`), and a defective declaration is never resolved against PATH at all.

### Consumer behavior

- **Strict validation.** `cflx openspec validate <id> --strict` and `--archive-gate` run the planner on the proposal's declaration list and report every defect as an error of the form `<change-id>: verification '<id>': <field>: <detail> (<code>)`. Conditions the validator already reports (missing non-empty fields, duplicate IDs, role conflicts) MUST remain reported at least once and MAY be reported by either path; the new effect is that `declaration_shell_syntax` and `declaration_missing_automation` on a required declaration now fail validation. Non-required declarations are never rejected by this rule, so an operational-observation `curl`/`docker compose` command remains valid.
- **`cflx openspec verify`.** In `--plan` mode, when the planner reports defects the command prints one line per defect (text) or a `declaration_defects` array alongside the existing report (JSON: `verification_id`, `field`, `code`, `detail`), executes nothing, and exits `1`. In run mode the same report is produced and **no** declared command is executed — not even a valid one — because the required set as a whole is invalid; existing evidence records are neither read for reuse nor modified, and the exit status is `1`. `--verification-id` selection does not narrow defect evaluation: duplicates and the required set are properties of the whole proposal. With zero defects both modes are unchanged.
- **Native Acceptance.** The manifest builder records the defects on the frozen manifest (a `declaration_defects` list, empty when valid) so that every persisted manifest and every injected projection can distinguish an invalid required manifest from a legitimately empty allowlist. When defects exist, the runtime-owned boundary settles as a typed **`fail` verdict about the candidate** (`AcceptanceResult::Fail`) recorded as an acceptance attempt against the candidate commit, carrying one finding per defect with the stable leading code `[DECLARATION_INVALID]`, the verification ID, the field, the code, the detail, and the declaration path `openspec/changes/<id>/proposal.md`. In that case no declared gate is executed or reused, no evidence is written, no reviewer subprocess is started, no `tool_unavailable` or external-prerequisite classification is produced, and no non-resumable execution hold is recorded. This is a verdict, so ordinary retry and the existing rejection/repair cycle apply unchanged, and the admission guard refuses nothing on its account. A legitimately empty allowlist keeps today's behavior: `Proceed`, reviewer launched with a closed empty boundary.
- **Error classification.** A declaration defect is a repository-local defect of the candidate's own `proposal.md`. It MUST NOT be classified as `tool_unavailable`, as a `blocked`/`stalled` external prerequisite, as `acceptance_runtime_defect`, or as `acceptance_manifest_malformed`.

### Explicit non-goals

The supervisable command syntax itself (the metacharacter set, the tokenizer, the absence of leading `NAME=value` assignments) is unchanged; extending it is the separate change `fix-rerun-leading-env`, which consumes the defect channel this change creates. No configuration switch, environment variable, or proposal field can bypass the planner. No implicit shell is introduced. The reuse policy, evidence record schema, reviewer prompt contents (other than not being built), and the `tool_unavailable` prerequisite contract are unchanged.

## Change boundary

- `src/orchestration/acceptance/execution_manifest.rs` (planner, manifest `declaration_defects`, fingerprint unchanged in meaning: the defects are part of the frozen manifest and therefore of its fingerprint), `src/orchestration/acceptance/verification_evidence.rs` (defect type, non-echoing `declaration_shell_syntax` detail), `src/orchestration/acceptance/manifest_builder.rs`, `src/orchestration/acceptance/gate_execution.rs` (new verdict variant), `src/parallel/executor.rs` (routing of the new verdict to `AcceptanceResult::Fail`), `src/openspec_cmd/validation.rs`, `src/openspec_cmd/verify.rs`.
- Docs: `docs/guides/VERIFICATION_EVIDENCE.md` ("What is eligible", exit-status paragraph, a new "Declaration defects" paragraph) and the verification authoring guidance in `skills/cflx-proposal/SKILL.md` (change-blocking commands must be a plain argv with no shell operators; duplicate IDs are rejected).
- Spec deltas: ADDED requirement in `proposal-metadata`; MODIFIED `Native validator owns behavior-centric proposal checks` in `cflx-proposal-validation` carried in full with one added clause and one added scenario.
- Out of scope: TUI/API schema changes (the `fail` verdict reaches them through existing fields), reviewer skill text, config.

## Acceptance

1. **Planner.** Production planner tests (`src/orchestration/acceptance/execution_manifest/tests.rs`) prove: one valid required declaration yields exactly one gate and zero defects; required declarations with `|`, `&&`, `;`, `>`, `$(…)`, an unbalanced quote, an empty command, and a missing automation path each yield exactly one defect with the frozen code and field and zero gates; two declarations sharing an ID with identical content and two sharing an ID with different commands each yield `declaration_duplicate_id` naming both positions and no gate for that ID; a required valid declaration next to a non-required declaration containing `$` yields one gate and zero defects; a list of only non-required or legacy declarations yields zero gates and zero defects; a change-blocking declaration with `phase: post-integration` or `execution_class: deployed-service` yields `declaration_role_conflict` on the right field.
2. **Validation and verify.** In-process tests through the production `validate` and `cmd_verify` entrypoints on a disposable change directory and Git repository, plus real-binary tests in `tests/run_exit_tests.rs` with the state root pointed outside the repository: a required declaration with shell syntax fails `--strict` and `--archive-gate` with the ID and field and a nonzero exit; `verify --plan` and `verify` exit `1`, JSON carries `declaration_defects`, and a fixture command that would create a marker file on execution leaves no marker; a pre-existing valid evidence record for another declaration is byte-identical afterwards; a non-required declaration with shell syntax produces no validation error.
3. **Native boundary.** Tests through the production `run_acceptance_boundary` and executor routing on a disposable Git repository with a counting AI-runner double: an invalid required manifest settles as `AcceptanceResult::Fail` whose findings carry `[DECLARATION_INVALID]`, the verification ID, and the field; the AI runner double records zero invocations; no gate process was launched (marker absent); the result is not `Stalled`, not `ExecutionHold`, and carries no `tool_unavailable`; the persisted `manifest.json` lists the defects. A single valid required declaration executes its gate exactly once (marker written once) and proceeds. A legitimately empty allowlist proceeds with the existing empty-boundary context.
4. **Gating.** Every new test name contains `required_verification_fail_closed`. Both declared commands select at least one test; zero selected tests is a failing gate. Any test exceeding one second is annotated `#[cfg_attr(not(feature = "heavy-tests"), ignore)]` and still runs under the declared commands because they enable `heavy-tests`. No test writes evidence, logs, or state inside the target repository or any managed worktree; every temporary state root is outside the test repository.
5. **Spec and docs coherence.** The `proposal-metadata` ADDED requirement and the carried-in-full MODIFIED `cflx-proposal-validation` requirement archive cleanly into the canonical specs without dropping any existing paragraph or scenario; the docs and skill text match the frozen codes. Strict authoring validation of this change is not implementation evidence; implementation evidence is the two declared gates with nonzero selected counts.

## Delivery boundary

Do not push, release, replace a live owner, or edit global or project configuration. Work only in the Conflux-managed candidate worktree of this independent develop-based repository; keep `main` untouched. Do not mark any task complete from prose, from a zero-test gate, or from an authoring-time strict validation pass.
