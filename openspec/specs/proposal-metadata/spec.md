## Requirements

### Requirement: proposal.md frontmatter metadata

`openspec/changes/<change-id>/proposal.md` MAY contain YAML frontmatter, and a proposal without frontmatter MUST remain readable. When frontmatter contains `verifications`, proposal tooling MUST parse it as an ordered list of structured verification declarations and MUST preserve the declarations when the proposal is read or archived. Runtime-supervised verification MUST resolve the declaration source from the active proposal when one exists, and otherwise from the sole canonical archive entry for the same logical change, using the repository's archive identity rules and failing closed when more than one canonical archive entry matches. Explicit verification metadata MUST remain authoritative over natural-language phase hints.

#### Scenario: proposal with verification metadata is accepted

**Given**: `proposal.md` contains valid frontmatter with pre-integration and post-integration verification declarations
**When**: proposal-aware tooling reads the proposal
**Then**: both declarations are retained with their original IDs, phases, owners, paths, evidence locations, rerun actions, and prerequisites
**And**: the proposal body remains available unchanged

#### Scenario: legacy proposal remains readable

**Given**: `proposal.md` has no frontmatter or no `verifications` field
**When**: tolerant proposal tooling reads it
**Then**: the proposal remains readable
**And**: no verification declaration is invented from prose

#### Scenario: archived proposal preserves declarations

**Given**: an active proposal contains valid verification declarations
**When**: the change is archived through the native archive command
**Then**: the archived `proposal.md` retains declarations equivalent to the active proposal

#### Scenario: archived change resolves the same verification declaration

**Given**: the active proposal is absent
**And**: exactly one canonical direct or dated archive entry contains the change proposal
**When**: runtime-supervised verification resolves the change by ID
**Then**: it reads the archived declaration with the same verification ID and repository-relative fields
**And**: it executes repository-local automation from the unchanged workspace root rather than from the archived directory

#### Scenario: an active proposal outranks a same-named archive entry

**Given**: an active proposal exists for the change ID
**And**: a canonical archive entry for the same change ID also exists
**When**: runtime-supervised verification resolves the change
**Then**: the active proposal is the declaration source
**And**: resolution does not fail on the coexistence

#### Scenario: multiple canonical archive entries fail closed

**Given**: the active proposal is absent
**And**: more than one canonical archive entry for the change ID contains `proposal.md`
**When**: runtime-supervised verification resolves the change
**Then**: resolution fails with an actionable ambiguity diagnostic naming the competing entries
**And**: no declaration or command is selected by filesystem iteration order

#### Scenario: invalid archive layout does not satisfy verification resolution

**Given**: only a nested date layout, malformed date, suffix collision, unrelated entry, or archive directory without `proposal.md` exists
**When**: runtime-supervised verification resolves the change
**Then**: the invalid entry is not accepted as the change's declaration source
**And**: a nested date layout reports the repository's existing invalid-archive-layout diagnostic
**And**: no verification command runs

### Requirement: proposal priority field

frontmatter の `priority` フィールドは proposal の優先度を表し、`high`、`medium`、`low` のいずれかでなければならない（MUST）。

#### Scenario: valid priority values are accepted

**Given**: `proposal.md` frontmatter に `priority: high` が含まれる
**When**: proposal metadata が解析される
**Then**: `priority` は有効な metadata として保持される

### Requirement: proposal dependencies field with backward compatibility

frontmatter の `dependencies` フィールドは change id の配列でなければならない（MUST）。`dependencies` が frontmatter に存在する場合、proposal tooling は本文 `## Dependencies` より frontmatter を優先しなければならない（MUST）。frontmatter に `dependencies` が存在しない場合、既存 proposal との後方互換のため本文 `## Dependencies` セクションを引き続き解釈しなければならない（MUST）。

#### Scenario: frontmatter dependencies override body section

**Given**: `proposal.md` frontmatter に `dependencies: ["base-change"]` がある
**And**: 本文 `## Dependencies` セクションには別の依存関係が書かれている
**When**: proposal dependencies を解析する
**Then**: `base-change` が依存関係として採用される
**And**: 本文 `## Dependencies` の値では上書きされない

#### Scenario: body dependencies remain supported without frontmatter field

**Given**: `proposal.md` に frontmatter の `dependencies` がない
**And**: 本文に `## Dependencies` セクションがある
**When**: proposal dependencies を解析する
**Then**: 本文 `## Dependencies` の change id 一覧が依存関係として採用される

#### Scenario: author guidance for archived dependency references

**Given**: active proposal の `dependencies` に含まれる change が archive に移動済みである
**When**: author が dependency metadata を見直す
**Then**: author は archived dependency reference が queued dependency と同一ではないことを認識できる
**And**: runtime/validation が archived reference を dedicated diagnostics で区別報告する前提で、依存が既に充足済みかどうかを proposal metadata から判断して更新可否を決められる

### Requirement: unknown frontmatter keys produce warnings

frontmatter に既知ではない key が含まれていても、proposal tooling は proposal の読み取りを失敗させてはならない（MUST NOT）。既知ではない key は warning として報告されなければならない（MUST）。

#### Scenario: unknown frontmatter key is warned but accepted

**Given**: `proposal.md` frontmatter に既知キーではない `owner: tumf` が含まれる
**When**: proposal metadata を解析または検証する
**Then**: proposal の読み取りは成功する
**And**: `owner` が unknown key である warning が報告される

### Requirement: proposal references field

frontmatter の `references` フィールドは文字列配列でなければならず、関連ファイル、spec、change id、その他の参照先を表現できなければならない（MUST）。proposal tooling は `references` をそのまま保持し、失われないように扱わなければならない（MUST）。

#### Scenario: references list preserves multiple targets

**Given**: `proposal.md` frontmatter に `references: ["src/openspec.rs", "openspec/specs/spec-only-changes/spec.md", "add-base-capability"]` がある
**When**: proposal metadata を解析する
**Then**: 3 件すべての reference が順序を保って保持される

> Canonical archive expectation: `proposal-metadata` capability は proposal frontmatter の形式・意味・後方互換ルールを canonical spec として保持する。

### Requirement: proposal verification declarations

Each verification declaration MUST contain a unique non-empty `id`, a non-empty `requirement`, a `phase`, an `owner`, a non-empty `trigger`, a safe repository-relative `automation` path, a non-empty `evidence` location, a non-empty `rerun` action, and a `prerequisites` string list. `phase` MUST be `pre-integration` or `post-integration`. A pre-integration declaration MUST use `owner: conflux-acceptance`; a post-integration declaration MUST use `owner: repository-automation`.

#### Scenario: pre-integration declaration identifies repository verification

**Given**: an implementation proposal declares `phase: pre-integration`
**When**: metadata is parsed
**Then**: the declaration identifies Conflux acceptance ownership and a tracked repository automation file

#### Scenario: post-integration declaration identifies operational ownership

**Given**: a proposal declares `phase: post-integration`
**When**: metadata is parsed
**Then**: the declaration identifies repository automation ownership, its trigger, evidence location, rerun action, and external prerequisites

#### Scenario: explicit phase wins over contradictory prose

**Given**: a structured declaration says `phase: post-integration`
**And**: proposal prose could be interpreted as pre-integration
**When**: tooling classifies the verification phase
**Then**: it uses `post-integration`
**And**: prose analysis may emit an advisory warning but cannot change routing semantics

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

### Requirement: Required verification declarations fail closed

Conflux MUST derive the Acceptance gate set from a proposal's ordered verification declarations through one shared declaration planner that returns both the eligible gate requests and an ordered list of typed declaration defects. The planner MUST be the single selection path for strict validation, `cflx openspec verify`, and the native Acceptance manifest; no consumer may select gates by discarding declarations that fail to convert into a supervisable request.

A declaration is *required* exactly when its structured metadata declares `completion_role: change-blocking`. Legacy declarations without role metadata and `operational-observation` declarations are non-required: they MUST produce neither gates nor defects, regardless of their command text.

The planner MUST report, in this order of evaluation: `declaration_duplicate_id` for every occurrence of an `id` that appears more than once in the list, with identical or different content, naming the ID and every position, with no gate produced for that ID and no deduplication; `declaration_role_conflict` for a required declaration whose `phase` is not `pre-integration` or whose `execution_class` is not `repository-local`; `declaration_missing_id` and `declaration_unstorable_id` for a required declaration without a safe single-component ID; `declaration_missing_command` for a required declaration whose `rerun` (or `evidence` when `rerun` is absent) is empty; `declaration_shell_syntax` for a required command containing any of `|`, `&`, `;`, `<`, `>`, `(`, `)`, `` ` ``, `$`, or that the POSIX-quote tokenizer cannot split, or that yields an empty argv or a blank argument; and `declaration_missing_automation` for a required declaration whose `automation` is absent, empty, absolute, or parent-traversing. Each defect MUST carry the verification ID (or the 1-based position when the ID is missing), the field, the stable code, and a detail; the `declaration_shell_syntax` detail MUST name the offending construct and MUST NOT echo the full command text. The planner MUST NOT resolve executables, read Git state, or access a network.

With zero defects the gates MUST be the required declarations' requests in canonical verification-ID order. With one or more defects the required manifest is *invalid*: no gate from it may be executed or reused by any consumer, and the outcome MUST be distinguishable from a legitimately empty allowlist (zero required declarations, zero defects).

`cflx openspec validate --strict` and `--archive-gate` MUST fail on every defect with a diagnostic naming the change, the verification ID, and the field. `cflx openspec verify --plan` and `cflx openspec verify` MUST report every defect (`declaration_defects` in JSON output), execute no declared command, leave existing evidence records unread and unmodified, and exit non-zero. Native Acceptance MUST record the defects on the frozen manifest and, when any exist, settle as a typed `fail` verdict about the candidate with one finding per defect carrying the leading code `[DECLARATION_INVALID]`, the verification ID, the field, the code, the detail, and the declaration path; it MUST NOT execute or reuse any gate, start a reviewer, write evidence, classify the outcome as `tool_unavailable` or as an external prerequisite, or record a non-resumable execution hold. No configuration, environment variable, or proposal field may bypass the planner, and no implicit shell may be introduced. The supervisable command syntax itself is unchanged by this requirement.

#### Scenario: Valid required declaration becomes exactly one gate

- **GIVEN** a proposal declares one `pre-integration`, `repository-local`, `change-blocking` verification whose command is a plain argv and whose automation path is tracked
- **WHEN** the declaration planner runs
- **THEN** it returns exactly one gate for that verification ID and zero defects
- **AND** native Acceptance executes that gate exactly once before the reviewer starts

#### Scenario: Shell syntax in a required declaration is a defect, not an omission

- **GIVEN** a required declaration whose `rerun` is `cargo test | tee out.log`, `cargo test && cargo clippy`, `cargo test > out.log`, or a command with an unbalanced quote
- **WHEN** the declaration planner runs
- **THEN** it returns a `declaration_shell_syntax` defect naming the verification ID, the field, and the offending construct without echoing the full command
- **AND** it returns no gate for that declaration
- **AND** the result is an invalid required manifest rather than an empty allowlist

#### Scenario: Duplicate IDs are rejected before any selection

- **GIVEN** two required declarations share the ID `local-tests`, in one case with identical content and in another case with different commands
- **WHEN** the declaration planner runs
- **THEN** it returns `declaration_duplicate_id` naming `local-tests` and both positions
- **AND** no gate is produced for `local-tests`
- **AND** no declaration is dropped by deduplication

#### Scenario: Non-required declarations and legitimately empty sets are preserved

- **GIVEN** a proposal declares one valid required verification and one `operational-observation` verification whose command contains `$`
- **WHEN** the declaration planner runs
- **THEN** it returns one gate and zero defects
- **AND** a proposal that declares only non-required or legacy declarations yields zero gates and zero defects, and Acceptance proceeds with a closed empty boundary

#### Scenario: Strict validation reports the defect before Apply

- **GIVEN** an active implementation proposal whose required declaration has shell syntax or a missing automation path
- **WHEN** `cflx openspec validate <id> --strict` or `--archive-gate` runs
- **THEN** validation fails with a diagnostic naming the change, the verification ID, and the field
- **AND** an operational-observation declaration with the same command text produces no error from this rule

#### Scenario: verify reports defects and runs nothing

- **GIVEN** a proposal with one defective required declaration and one valid required declaration that would create a marker file when executed
- **WHEN** `cflx openspec verify <id> --plan` or `cflx openspec verify <id>` runs
- **THEN** the output reports the defect with its verification ID, field, and code
- **AND** no declared command is executed and no marker file exists
- **AND** a pre-existing valid evidence record for the other declaration is unchanged
- **AND** the exit status is non-zero

#### Scenario: Native Acceptance fails closed without a reviewer

- **GIVEN** a managed candidate whose proposal declares a defective required verification
- **WHEN** the runtime-owned Acceptance boundary is derived
- **THEN** the frozen manifest records the defect
- **AND** Acceptance settles as a `fail` verdict whose findings carry `[DECLARATION_INVALID]`, the verification ID, the field, and `openspec/changes/<id>/proposal.md`
- **AND** no reviewer subprocess is started, no gate is executed or reused, and no evidence is written
- **AND** the outcome is neither `tool_unavailable`, nor an external-prerequisite block, nor a non-resumable execution hold

#### Scenario: Declaration defects are never infrastructure

- **GIVEN** a required declaration whose command is defective and whose first token would not resolve on PATH
- **WHEN** Acceptance classifies the outcome
- **THEN** the classification is the declaration defect
- **AND** the executable is never resolved and no `tool_unavailable` prerequisite is reported

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
