## MODIFIED Requirements

### Requirement: Required verification declarations fail closed

Conflux MUST derive the Acceptance gate set from a proposal's ordered verification declarations through one fallible verification-source reader and one shared declaration planner. The reader returns the decoded declarations together with an ordered list of typed source defects; the planner returns both the eligible gate requests and an ordered list of typed declaration defects. Together they are the single selection path for strict validation, `cflx openspec verify`, and the native Acceptance manifest; no consumer may select gates by discarding declarations that fail to decode or fail to convert into a supervisable request, and no consumer may derive an authoritative gate list from a tolerant reader that substitutes an empty list for a decode failure.

A declaration is *required* exactly when its structured metadata declares `completion_role: change-blocking`. Legacy declarations without role metadata and `operational-observation` declarations are non-required: they MUST produce neither gates nor defects, regardless of their command text.

The reader MUST report `declaration_metadata_invalid` for malformed frontmatter YAML, an opening frontmatter delimiter with no closing delimiter, a frontmatter root that is neither a mapping nor null, a `verifications` value that is neither a sequence nor null, a declaration entry that is not a mapping, a known declaration field of the wrong type, and an unknown declaration field. When any such defect exists the decoded declaration list is empty and the planner rules below are not evaluated. A readable proposal with no frontmatter, no `verifications` key, a null `verifications` value, or `verifications: []` is a legitimately empty source with zero defects. Unknown top-level frontmatter keys remain warnings. A source defect's detail MUST describe only the shape, the expected and found YAML types, and the line/column location; it MUST NOT echo field values, command text, environment values, YAML fragments, or the decoder's own error text.

The planner MUST report, in this order of evaluation: `declaration_duplicate_id` for every occurrence of a trimmed `id` that appears more than once in the list, with identical or different content, naming the ID and every position, with no gate produced for that ID and no deduplication; `declaration_role_conflict` for a required declaration whose `phase` is not `pre-integration` or whose `execution_class` is not `repository-local`; `declaration_missing_id` and `declaration_unstorable_id` for a required declaration without a safe single-component ID; `declaration_missing_command` for a required declaration whose `rerun` (or `evidence` when `rerun` is absent) is empty; `declaration_shell_syntax` for a required command containing any of `|`, `&`, `;`, `<`, `>`, `(`, `)`, `` ` ``, `$`, or that the POSIX-quote tokenizer cannot split, or that yields an empty argv or a blank argument; and `declaration_missing_automation` for a required declaration whose `automation` is absent, empty, absolute, or parent-traversing. Each defect MUST carry a typed declaration key fixed at construction from the source's own provenance (the trimmed authored ID when an `id` string with non-empty trim was written, whether or not that ID is storable or resembles a fallback label; the 1-based position when none was; the proposal itself when a source defect is not attributable to one declaration), together with the display label derived from that key (the trimmed ID, `<position N>`, or `<proposal>`), the position (`0` for a proposal-scoped source defect), the field (`frontmatter` or `verifications` for a proposal-scoped source defect), the stable code, and a detail; the key MUST NOT be reconstructed from the label, the position, the code, or any prose; the `declaration_shell_syntax` detail MUST name the offending construct and MUST NOT echo the full command text. Neither the reader nor the planner may resolve executables, read Git state, or access a network.

With zero defects the gates MUST be the required declarations' requests in canonical verification-ID order. With one or more source or planner defects the required manifest is *invalid*: no gate from it may be executed or reused by any consumer, no valid subset may run, `--verification-id` may not narrow defect evaluation, and the outcome MUST be distinguishable from a legitimately empty allowlist (zero required declarations, zero defects).

A proposal that cannot be read at all is *source-unavailable*, which is distinct from both an invalid and an empty manifest. `cflx openspec verify` MUST report it through its existing proposal-resolution error and exit non-zero before resolving the evidence store. Native Acceptance MUST refuse to derive a manifest, settling as the existing manifest-build execution hold with an all-zero fingerprint and an evidence line naming the proposal path and the I/O error kind; it MUST NOT proceed to a reviewer, run or reuse a gate, write evidence, settle a verdict, or classify the condition as `tool_unavailable` or an external prerequisite, and the next attempt remains admissible without a repository change.

`cflx openspec validate --strict` and `--archive-gate` MUST fail on every source and planner defect with a diagnostic naming the change, the verification ID, and the field, using the trimmed ID identity the planner uses, and MUST NOT suppress a duplicate-ID defect on the grounds that another check already reported it. `cflx openspec verify --plan` and `cflx openspec verify` MUST report every defect (`declaration_defects` in JSON output, each with its verification ID, position, field, code, and detail unchanged, plus an additive typed `key` member), execute no declared command, leave existing evidence records unread and unmodified, and exit non-zero. Native Acceptance MUST record the defects on the frozen manifest and, when any exist, settle as a typed `fail` verdict about the candidate with one finding per defect carrying the leading code `[DECLARATION_INVALID]`, the verification ID, the field, the code, the detail, and the declaration path; it MUST NOT execute or reuse any gate, start a reviewer, write evidence, classify the outcome as `tool_unavailable` or as an external prerequisite, or record a non-resumable execution hold. No configuration, environment variable, or proposal field may bypass the reader or the planner, and no implicit shell may be introduced. The supervisable command syntax itself is unchanged by this requirement.

Each native declaration finding MUST carry a typed identity, separate from its human text, composed of the change ID, the declaration key, the field, and the code, encoded so that a user-authored ID can never collide with a positional or proposal-scoped fallback key. The key is the defect's own typed declaration key, copied as constructed by the reader or planner: the trimmed verification ID for a named declaration, the 1-based position for a nameless one, and the proposal for a source-wide defect. The identity MUST NOT be derived by comparing the display label against a fallback pattern, by inspecting the position, or by parsing prose, so an authored ID that is literally spelled like a fallback label keeps its named identity. Finding normalization MUST use that identity before any reviewer-ID or text-derived fallback, so list reordering, line numbers, detail wording, and the candidate revision never change it, while a different field, code, or key is a distinct identity. Occurrences sharing one identity share one automatic repair opportunity under the existing in-process ledger and keep every complete finding visible; the same still-open identity observed again after its repair MUST still stop. A native declaration finding MUST NOT acquire reviewer-authored remediation coverage obligations, and the identity MUST NOT be persisted as workflow authority: after an owner restart it is recomputed from the candidate's `proposal.md`.

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

#### Scenario: A wrong-typed required field never becomes a successful empty plan

- **GIVEN** a proposal whose change-blocking declaration has `rerun: [cargo, test]`
- **WHEN** `cflx openspec verify <id> --plan`, `cflx openspec verify <id>`, or the native Acceptance boundary reads it
- **THEN** a `declaration_metadata_invalid` defect names that declaration's ID and the field `rerun` with a detail stating a sequence was found where a string was expected
- **AND** verify exits non-zero with `declaration_defects` in JSON output and executes nothing
- **AND** native Acceptance records the defect on the manifest and settles `fail` before any gate or reviewer
- **AND** existing evidence is unchanged and the detail does not contain `cargo`

#### Scenario: Malformed candidate metadata is caught after Apply, not only before it

- **GIVEN** a change whose base proposal passed `--strict` validation
- **AND** the Apply phase committed a candidate whose `proposal.md` now has an opened frontmatter block with no closing delimiter, a non-mapping frontmatter root, `verifications: true`, a declaration entry that is a plain string, or a declaration carrying an unknown field
- **WHEN** the native Acceptance boundary is derived from that candidate
- **THEN** the manifest carries a `declaration_metadata_invalid` defect bound to the candidate commit, proposal-scoped with field `frontmatter` or `verifications` when no declaration can be identified and keyed by the declaration ID or position otherwise
- **AND** Acceptance settles `fail` with one `[DECLARATION_INVALID]` finding per defect, starts no reviewer, and runs or reuses no gate
- **AND** the earlier base validation result is not consulted as evidence about the candidate

#### Scenario: Trimmed duplicate IDs are rejected by every consumer

- **GIVEN** two required declarations whose IDs are `gate` and `" gate "`
- **WHEN** `cflx openspec validate <id> --strict`, `--archive-gate`, `cflx openspec verify <id> --plan`, or the native Acceptance boundary plans them
- **THEN** each reports `declaration_duplicate_id` for `gate` naming both positions
- **AND** validation exits non-zero rather than treating the two as distinct or suppressing the planner's defect as already reported

#### Scenario: Source failure and valid emptiness stay distinct

- **GIVEN** a managed candidate whose `openspec/changes/<id>/proposal.md` is missing or cannot be read
- **WHEN** the native Acceptance boundary is derived
- **THEN** no manifest is produced and the boundary settles as the existing manifest-build execution hold whose evidence names the proposal path and the I/O error kind
- **AND** no reviewer is started, no gate runs or is reused, no evidence is written, and no `pass`, `fail`, or `tool_unavailable` classification is recorded
- **AND** `cflx openspec verify <id>` against the same workspace exits non-zero through its proposal-resolution error without resolving the evidence store
- **AND** a readable proposal with no frontmatter, no `verifications` key, a null `verifications` value, or `verifications: []` still yields zero gates and zero defects and proceeds to the reviewer

#### Scenario: Repairing one defect exposes a different repairable defect

- **GIVEN** a candidate whose declaration `gate` is duplicated, so the `fail` verdict carries two `[DECLARATION_INVALID]` findings that normalize to one identity for `gate`, field `id`, code `declaration_duplicate_id`
- **AND** the automatic repair for that identity has been dispatched
- **WHEN** the repaired candidate removes the duplicate and the next verdict reports `gate`, field `rerun`, code `declaration_shell_syntax`
- **THEN** the repair ledger grants that new identity its first repair opportunity
- **AND** a verdict that reports the same still-open `gate` / `id` / `declaration_duplicate_id` identity after its repair stops with the repeated-finding reason
- **AND** reordering `gate` in the list, changing its line number, changing the detail wording, or producing a new candidate commit does not change its identity

#### Scenario: An authored ID spelled like a fallback label keeps its own identity

- **GIVEN** a candidate whose third declaration carries the authored `id` `<position 3>` and a `rerun` with shell syntax, and another candidate whose third declaration has no `id` and the same `rerun`
- **WHEN** the production planner builds the defects and native Acceptance constructs and normalizes the findings
- **THEN** the first defect is keyed by the named ID `<position 3>` and the second by position 3, and their identities differ although field, code, label text, and position are equal
- **AND** moving the named declaration to position 1 does not change its identity, while the JSON `key` member reports `id` for the first and `position` for the second
- **AND** an authored ID that is unstorable is still keyed by that ID in its `declaration_unstorable_id` defect and finding
- **AND** reviewer-authored finding identities and coverage behavior in the same process are unchanged

#### Scenario: Native declaration findings carry no reviewer coverage obligations

- **GIVEN** a `fail` verdict whose findings were constructed by the native declaration path
- **WHEN** the findings are inspected for remediation coverage and for a reviewer-authored ID
- **THEN** they declare no required implementation or verification paths and no reviewer ID
- **AND** they are classified as repository-scoped, never as external prerequisites
- **AND** a structured reviewer finding in the same process keeps its reviewer-ID identity and its coverage behavior unchanged
