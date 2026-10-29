## ADDED Requirements

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
