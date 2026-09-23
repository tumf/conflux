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

Conflux MAY reuse successful Apply or runtime-owned Acceptance verification evidence during Acceptance only from a versioned Conflux-owned external cache created by a Conflux-runtime-owned executor that directly supervised the declared process and bound the result to one declared verification ID. The cache MUST resolve outside the target repository and every managed worktree under the configured or platform-default Conflux state root, isolated by project, workspace, and change identity. An agent-authored record MUST NOT be reusable. The version-2 record MUST include the full-length candidate commit object ID and tree ID, exact review base and range, exact argv, repository-relative working directory, tracked automation-file blob identity, tool executable identity, start and end timestamps, exit code, store-relative evidence artifact path and content digest, and clean index/worktree state before and after execution. Acceptance MUST compare every binding with current Git state, the current proposal declaration, and the runtime-owned execution manifest while evaluating target cleanliness without Conflux-path exclusions. A missing, malformed, stale, dirty, mismatched, unsuccessful, agent-authored, deleted, or unverifiable cache record MUST force runtime-owned command execution or a typed hold and MUST NOT imply PASS. Absence or invalidity of cache causes rerun only and never changes the next workflow action. External records MUST remain cache and diagnostics only and MUST NOT control admission, retry, archive, merge, or next-action selection. An unavailable external store or a store resolving inside the target repository or any managed worktree MUST fail closed before command execution and MUST NOT fall back to a target-worktree path.

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
