## ADDED Requirements

### Requirement: Declared verification commands admit leading environment assignments

A declared verification command (`rerun`, or `evidence` when `rerun` is absent) MUST be tokenized with the existing shell-metacharacter refusal and POSIX-quote tokenizer, after which leading tokens of the form `NAME=value` with `NAME` matching `[A-Za-z_][A-Za-z0-9_]*` MUST be consumed in order as environment assignments. The first token that is not an assignment is the executable; later tokens are ordinary arguments even when they contain `=`. The value is everything after the first `=`; it MAY be empty and MAY contain `=`, spaces, or quotes supplied through the tokenizer. Multiple assignments are allowed; when a name repeats, the last assignment wins and the effective bindings are the resulting map. A leading token containing `=` whose name is not a valid identifier MUST be the declaration defect `declaration_invalid_assignment`; a command consisting only of assignments MUST be the declaration defect `declaration_missing_command`; both are repository-local declaration defects under the fail-closed declaration contract and MUST NOT be classified as `tool_unavailable` or as an external prerequisite. Expansion, substitution, redirection, pipelines, conditionals, and compound commands remain `declaration_shell_syntax`. `env NAME=value cmd` MUST remain an ordinary argv whose executable is `env`.

Bindings MUST be applied to the supervised child process only, on top of the environment it would otherwise inherit, and MUST NOT mutate the owner process environment or introduce an intermediary shell. Tool resolution, tool identity, and the `tool_unavailable` prerequisite MUST use the executable token only. The effective bindings MUST enter gate identity and evidence identity by digest and by name only: the frozen manifest gate and the evidence record carry `env_names` (sorted, distinct) and `env_binding_digest` (full-length SHA-256 over name-sorted `name NUL value NUL` pairs), the manifest fingerprint includes both, and evidence reuse compares `env_binding_digest` as one more all-fields binding. No persisted record, manifest, log, event, API field, TUI line, reviewer context, or `verify` report MAY contain an assignment value; every rendered command MUST show bindings as `NAME=<redacted>` followed by the executable argv. This restriction applies to Conflux-generated command metadata, not arbitrary raw child stdout/stderr. A child can deliberately print a value into the native gate output artifact, which is not promised safe for sharing; use a separate non-echoing fixture for metadata sentinel-absence tests.

#### Scenario: Real verifier executes the POSIX prefix form

- **GIVEN** a required declaration whose command is `DJANGO_SETTINGS_MODULE=config.test_settings printenv DJANGO_SETTINGS_MODULE`
- **WHEN** the runtime executor supervises the gate
- **THEN** `printenv` is the resolved executable, the child exits `0`, and the captured artifact contains `config.test_settings`
- **AND** the owner process environment does not contain `DJANGO_SETTINGS_MODULE` afterwards
- **AND** the same command without the assignment exits non-zero

#### Scenario: Quoting, empty values, duplicates, and explicit env

- **GIVEN** declared commands `'A=hello world' cmd`, `A=x=y cmd`, `A= cmd`, `A=1 B=2 cmd`, `A=1 A=2 cmd`, `env A=1 cmd`, and `cmd a=b`
- **WHEN** the declaration planner converts them to requests
- **THEN** the bindings are respectively `A=hello world`, `A=x=y`, `A=` (empty), `A=1` and `B=2`, `A=2`, none (executable `env` with argument `A=1`), and none (argument `a=b` retained)
- **AND** the executable argv never contains an assignment token

#### Scenario: Invalid assignment and assignment-only commands are declaration defects

- **GIVEN** declared commands `1ABC=x cmd`, `A-B=x cmd`, and `A=1`
- **WHEN** the declaration planner runs
- **THEN** the first two are `declaration_invalid_assignment` and the third is `declaration_missing_command`, each naming the verification ID and the field
- **AND** strict validation fails, `cflx openspec verify` exits non-zero, and native Acceptance settles as a `fail` verdict with `[DECLARATION_INVALID]`
- **AND** none is reported as `tool_unavailable` or as an external prerequisite

#### Scenario: Missing actual executable remains infrastructure

- **GIVEN** a required declaration `A=1 definitely-not-installed-tool --flag`
- **WHEN** the manifest is frozen and the gate phase runs
- **THEN** the outcome is the existing `tool_unavailable` resumable external prerequisite naming the executable
- **AND** the assignment is not named as the missing tool

#### Scenario: Changing a binding invalidates prior evidence

- **GIVEN** an exact evidence record bound to a gate whose effective bindings were `A=1`
- **WHEN** the declaration changes to `A=2`, adds `B=1`, or drops the assignment
- **THEN** the manifest fingerprint differs
- **AND** the record decides `Rerun` with mismatch field `env_binding_digest`
- **AND** an envelope written before the field existed matches an assignment-free declaration and reruns for one with assignments

#### Scenario: Assignment values are never rendered or persisted

- **GIVEN** a declaration whose assignment value is a unique sentinel
- **WHEN** the gate is planned, executed, reused, reported by `verify`, injected into the reviewer boundary context, logged, projected into a hold or `error_detail`, or rendered in the TUI
- **THEN** the sentinel appears in no Conflux-generated command metadata, manifest/envelope JSON or display; the test uses a non-echoing child, and raw child output is not covered by this guarantee
- **AND** `env_names` lists the name and the rendered command shows `NAME=<redacted>`

## MODIFIED Requirements

### Requirement: Verification evidence reuse is bound to current repository state

Conflux MAY reuse successful Apply or runtime-owned Acceptance verification evidence during Acceptance only from a versioned Conflux-owned external cache created by a Conflux-runtime-owned executor that directly supervised the declared process and bound the result to one declared verification ID. The cache MUST resolve outside the target repository and every managed worktree under the configured or platform-default Conflux state root, isolated by project, workspace, and change identity. An agent-authored record MUST NOT be reusable. The version-2 record MUST include the full-length candidate commit object ID and tree ID, exact review base and range, exact executable argv, the effective environment binding names and their full-length binding digest (never a value), repository-relative working directory, tracked automation-file blob identity, tool executable identity, start and end timestamps, exit code, store-relative evidence artifact path and content digest, and clean index/worktree state before and after execution. Acceptance MUST compare every binding with current Git state, the current proposal declaration, and the runtime-owned execution manifest while evaluating target cleanliness without Conflux-path exclusions. A missing, malformed, stale, dirty, mismatched, unsuccessful, agent-authored, deleted, or unverifiable cache record MUST force runtime-owned command execution or a typed hold and MUST NOT imply PASS. Absence or invalidity of cache causes rerun only and never changes the next workflow action. External records MUST remain cache and diagnostics only and MUST NOT control admission, retry, archive, merge, or next-action selection. An unavailable external store or a store resolving inside the target repository or any managed worktree MUST fail closed before command execution and MUST NOT fall back to a target-worktree path.

The environment binding digest is one more all-fields binding: an envelope whose `env_binding_digest` differs from the current declaration's effective bindings MUST rerun with mismatch field `env_binding_digest`, and an envelope lacking the field MUST be read as the digest of empty bindings.

#### Scenario: Exact successful evidence is reused

**Given**: a runtime-authored external evidence cache record identifies one change-blocking verification
**And**: every commit/tree, command, cwd, automation blob, tool, artifact, exit, and clean-state binding matches the current worktree and declaration
**When**: Acceptance evaluates that verification
**Then**: Acceptance may reuse the successful result without executing the same command
**And**: output identifies the verification as reused

#### Scenario: Stale commit evidence reruns

**Given**: otherwise valid evidence is bound to a different commit or tree
**When**: Acceptance evaluates the current result
**Then**: it reruns the declared verification
**And**: it does not infer PASS from the stale record

#### Scenario: Command or tool mismatch reruns

**Given**: evidence argv, cwd, automation blob, or tool identity differs from the current declaration or executable
**When**: Acceptance evaluates the record
**Then**: it reruns the verification with an actionable mismatch reason

#### Scenario: Dirty capture is never reusable

**Given**: a record says the index or worktree was dirty at capture, or current cleanliness cannot be proven
**When**: Acceptance evaluates the record
**Then**: the record is ineligible for reuse
**And**: Acceptance reruns or reports the ordinary execution blocker

#### Scenario: Evidence cache does not alter target cleanliness

**Given**: the current commit and tree match the bound candidate result
**And**: exact runtime-authored evidence exists only in the external Conflux state root
**When**: Acceptance evaluates the runtime-authored record
**Then**: target cleanliness is evaluated without excluding any Conflux-owned path
**And**: any target difference still forces rerun or a typed hold

#### Scenario: Apply-authored evidence is not authority

**Given**: an envelope was written or modified without a matching runtime-supervised execution
**When**: Acceptance evaluates the envelope
**Then**: the envelope is ineligible for reuse
**And**: Acceptance reruns the declared verification

#### Scenario: Malformed evidence fails closed to rerun

**Given**: a record is missing a required field, uses a short commit ID, has an invalid digest, or references a missing artifact
**When**: Acceptance parses it
**Then**: the record does not satisfy verification
**And**: the declared command remains the source of truth

#### Scenario: Restart does not promote external cache to workflow authority

**Given**: Conflux restarts while external cache records are present, absent, or deleted
**When**: Acceptance reconstructs the next action
**Then**: repository and Git state determine the next workflow action
**And**: valid cache may reuse a gate result but cannot refuse admission or imply PASS

#### Scenario: Review range mismatch invalidates evidence

**Given**: otherwise valid evidence is bound to a different candidate revision, review base, or review range
**When**: Acceptance constructs the current execution manifest
**Then**: the evidence is not reusable
**And**: the runtime executes the declared verification or returns its ordinary typed hold

#### Scenario: Undeclared command has no blocking authority

**Given**: the execution manifest contains every eligible change-blocking declaration
**When**: a semantic reviewer proposes an undeclared repository-wide command
**Then**: that command is not started or stored as blocking evidence
**And**: its absence cannot prevent a verdict

#### Scenario: External state failure does not pollute the target

**Given**: the configured or derived Acceptance state path cannot be created or written
**When**: runtime prepares a declared gate
**Then**: Acceptance returns a typed runtime hold before launching the gate
**And**: it creates no fallback directory or file in the target repository or worktree

#### Scenario: Legacy target-local evidence is not hidden or migrated

**Given**: a target contains legacy `.cflx/verification-evidence` content
**When**: the new runtime evaluates cleanliness or evidence reuse
**Then**: it does not open, parse, migrate, rewrite, hide, or delete that content
**And**: a dedicated existence probe reports the path for explicit repository-owner cleanup without evaluating unrelated ignored paths

#### Scenario: Environment binding mismatch reruns

**Given**: otherwise valid evidence whose `env_binding_digest` was recorded for different effective bindings than the current declaration's
**When**: Acceptance evaluates the record
**Then**: it reruns the verification with mismatch field `env_binding_digest`
**And**: no assignment value appears in the mismatch detail
