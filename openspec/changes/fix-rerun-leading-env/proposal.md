---
change_type: implementation
priority: high
dependencies:
  - fix-required-verification-fail-closed
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/proposal-metadata/spec.md
  - openspec/specs/process-execution/spec.md
  - docs/guides/VERIFICATION_EVIDENCE.md
  - src/orchestration/acceptance/verification_evidence.rs
  - src/orchestration/acceptance/execution_manifest.rs
  - src/orchestration/acceptance/gate_execution.rs
verifications:
  - id: fix-rerun-leading-env-local
    requirement: Leading NAME=value assignments in a declared verification command are parsed into child-only environment bindings, resolved against the real executable, bound into gate and evidence identity by digest, and never rendered in plaintext
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/orchestration/acceptance/verification_evidence.rs
    evidence: cargo test --lib --features web-monitoring,heavy-tests rerun_leading_env -- --nocapture
    rerun: cargo test --lib --features web-monitoring,heavy-tests rerun_leading_env -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: fix-rerun-leading-env-cli
    requirement: The real cflx binary runs a declared command with a leading assignment through openspec verify, the child observes the value, the owner environment is unchanged, and Conflux-generated command metadata contains no assignment value (raw child output is a distinct artifact)
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: cargo test --test run_exit_tests --features web-monitoring,heavy-tests rerun_leading_env -- --nocapture
    rerun: cargo test --test run_exit_tests --features web-monitoring,heavy-tests rerun_leading_env -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Support leading environment assignments in declared verification commands

**Change Type**: implementation

## Problem

A declared `rerun` such as `DJANGO_SETTINGS_MODULE=config.test_settings uv run pytest` is split by `eligible_request` (`src/orchestration/acceptance/verification_evidence.rs`) into a plain argv whose first token is the assignment. The manifest builder then resolves `DJANGO_SETTINGS_MODULE=config.test_settings` as the executable, `resolve_tool` fails, the manifest's tool path is empty, and `execute_declared_gates` reports `tool_unavailable`. `classify_gate_results` turns that into a *resumable external prerequisite* (`infrastructure`), so native Acceptance stops before the reviewer starts and tells the operator to install a tool that was never a tool. Rewriting the command as `env DJANGO_SETTINGS_MODULE=… uv run pytest` is a verified workaround, not the product fix: the POSIX form is what proposal authors write, and the runtime must either support it or refuse it as a declaration defect — never misreport it as missing infrastructure.

## Supported result

### Syntax (frozen)

A declared verification command (`rerun`, or `evidence` when `rerun` is absent) is tokenized exactly as today: the shell-metacharacter refusal (`|`, `&`, `;`, `<`, `>`, `(`, `)`, `` ` ``, `$`) and the POSIX-quote tokenizer are unchanged. On the *tokenized* argv, leading tokens that match `NAME=value` with `NAME` matching `[A-Za-z_][A-Za-z0-9_]*` are consumed, in order, as environment assignments. The first token that does not match is the executable; every later token is an ordinary argument even when it contains `=`. The value is everything after the first `=`; it may be empty and may itself contain `=`, spaces, or quotes supplied through the tokenizer (`'A=hello world' cmd` binds `A` to `hello world`). Multiple assignments are allowed; when a name repeats, the last assignment wins and the *effective bindings* are the resulting name-to-value map.

Two new declaration defects extend the planner introduced by `fix-required-verification-fail-closed`, both on field `rerun` (or `evidence`): `declaration_invalid_assignment` when a leading token contains `=` before the executable but its name part is not a valid identifier (for example `1ABC=x cmd`, `A-B=x cmd`, `=x cmd`); and `declaration_missing_command` when every token is an assignment and no executable remains. These are repository-local declaration defects with the classification that change defines; they are never `tool_unavailable` and never an external prerequisite. Expansion (`$VAR`), command substitution, redirection, pipelines, conditionals, and compound commands remain refused as `declaration_shell_syntax`. `env NAME=value cmd` is unchanged: `env` is the executable, the bindings are empty, and `env`'s own identity is the tool identity.

### Execution

`VerificationRequest` carries the effective bindings as an ordered list of `(name, value)` pairs alongside the executable argv (executable plus arguments only; assignments are never part of `argv`). The runtime supervisor applies the bindings to the spawned child only, on top of the environment the child would otherwise inherit, through the process builder's per-child environment — never by mutating the owner process environment, never through `sh -c`, `env`, or any other intermediary. Tool resolution, tool identity, and the `tool_unavailable` prerequisite use the executable token only, so a missing *actual* executable is still the existing infrastructure classification.

### Identity

Gate identity and evidence identity include the effective bindings by **digest, never by value**. The frozen manifest gate and the version-2 evidence record gain `env_names` (the sorted, distinct effective names) and `env_binding_digest` (a full-length SHA-256 hex digest over the canonical effective bindings: pairs sorted by name, each encoded as `name`, a NUL byte, `value`, a NUL byte). The manifest fingerprint includes both; evidence reuse adds `env_binding_digest` to the all-fields conjunction with mismatch field `env_binding_digest`; an envelope written before this field existed reads as the digest of empty bindings, so it still matches a declaration without assignments and reruns for one with assignments. Changing a value, adding a name, or removing a name therefore invalidates prior evidence and changes the Acceptance fingerprint. No persisted record, manifest, log line, event, API field, TUI line, reviewer context, or `verify` report contains an assignment value; the digest is computed in memory from the declaration alone.

### Rendering and redaction

Every place a declared command, request, or gate is rendered — reuse decision summaries, `verify` text and JSON reports, gate outcome summaries, the Acceptance boundary context injected into the reviewer prompt, execution-hold evidence lines, logs, and API/TUI diagnostics — renders bindings as `NAME=<redacted>` (names only) followed by the executable argv. A diagnostic for a command that could not be tokenized never echoes the command text (already required by `fix-required-verification-fail-closed`). This is a guarantee about Conflux-generated command metadata, not a promise to scrub arbitrary child stdout/stderr: a child that deliberately prints a value (such as the printenv success fixture) produces that output in the native gate artifact. Use a separate non-echoing child for sentinel-absence assertions. Do not corrupt raw verification output or claim it is safe for unrestricted sharing.

### Explicit non-goals

No shell semantics beyond leading assignments: no expansion, substitution, redirection, globbing, `~`, or compound commands. No configuration switch. No change to the reuse duration threshold, the external store layout, or the reviewer skill's authority. Argument values other than assignments are not redacted by this change (that is the separate `improve-acceptance-provenance` display contract); they are also not newly exposed anywhere.

## Change boundary

- `src/orchestration/acceptance/verification_evidence.rs`: request parsing (assignments → bindings), `VerificationEvidence` fields, `CommandSupervisor` and `DirectCommandSupervisor` (child environment), reuse comparison, redacting renderer.
- `src/orchestration/acceptance/execution_manifest.rs` / `manifest_builder.rs`: `ManifestGate` fields and fingerprint; planner defects `declaration_invalid_assignment`.
- `src/orchestration/acceptance/gate_execution.rs`, `src/agent.rs` boundary context, `src/openspec_cmd/verify.rs`, `src/openspec_cmd/validation.rs`: use the renderer; validation reports the two new defects through the shared planner.
- Docs: `docs/guides/VERIFICATION_EVIDENCE.md` ("What gets bound" table gains an environment-bindings row; "What is eligible" documents the assignment prefix and what is still refused), `skills/cflx-proposal/SKILL.md` (authoring syntax for `rerun`/`evidence`: optional leading `NAME=value` assignments, then a plain argv; what is refused), `skills/cflx-accept/SKILL.md` (gate rows show `NAME=<redacted>`; the reviewer never needs the value).
- Spec deltas: ADDED requirement and MODIFIED `Verification evidence reuse is bound to current repository state` (carried in full) in `proposal-metadata`; ADDED requirement in `process-execution`.

## Acceptance

1. **Real execution.** A production-supervisor test (`DirectCommandSupervisor`, real child process) in `--lib` and a real-binary `cflx openspec verify` test in `tests/run_exit_tests.rs` run the declared command `DJANGO_SETTINGS_MODULE=config.test_settings printenv DJANGO_SETTINGS_MODULE` on a disposable Git repository with the state root outside it: the gate exits `0`, the captured artifact contains `config.test_settings`, the same command without the assignment exits non-zero, and the test process's own `DJANGO_SETTINGS_MODULE` remains unset afterwards. `printenv` is the portable fixture; no shell is involved.
2. **Parsing coverage.** Planner/request tests cover: quoted value with spaces, value containing `=`, empty value, multiple assignments, a repeated name where the last wins, `env A=b cmd` as ordinary argv with empty bindings, a later `a=b` argument kept as an argument, an invalid identifier before the executable (`declaration_invalid_assignment`), an assignment-only command (`declaration_missing_command`), and `FOO=$BAR cmd`, pipelines, and redirections still refused as `declaration_shell_syntax`.
3. **Classification and identity.** A missing actual executable after valid assignments still reports `tool_unavailable` with the external-prerequisite contract; an invalid assignment is a declaration defect through the fail-closed path (strict validation error, `verify` non-zero, Acceptance `fail` with `[DECLARATION_INVALID]`). Changing a binding value, adding or removing a name changes the manifest fingerprint and makes an otherwise exact evidence record decide `Rerun` with mismatch field `env_binding_digest`; a pre-field envelope matches an assignment-free declaration and reruns for one with assignments.
4. **No plaintext leak.** With a value chosen as a unique sentinel, tests assert the sentinel is absent from: the stored envelope JSON and `manifest.json`, `verify --plan`/`verify` stdout and stderr (text and `--json`), reuse decision summaries, gate outcome summaries, the injected reviewer boundary context, execution-hold evidence lines, and the `/api/v2` `error_detail` projection of a gate failure; `env_names` contains the name.
5. **Gating.** Every new test name contains `rerun_leading_env`; both declared commands select at least one test (zero selected is a failing gate); tests over one second are `#[cfg_attr(not(feature = "heavy-tests"), ignore)]`; no test writes into the target repository or any managed worktree.
6. **Spec and docs coherence.** The MODIFIED reuse requirement is carried in full with only the environment-binding additions; the ADDED requirements archive cleanly; docs and skills describe the same syntax and codes the tests prove. Strict validation of this change is not implementation evidence.

## Delivery boundary

Do not push, release, replace a live owner, or edit global or project configuration. Work only in the Conflux-managed candidate worktree of this independent develop-based repository; keep `main` untouched. Do not mark any task complete from prose, from a zero-test gate, or from an authoring-time strict validation pass.
