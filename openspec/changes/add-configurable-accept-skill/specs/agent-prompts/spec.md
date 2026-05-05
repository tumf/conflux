## MODIFIED Requirements

### Requirement: Acceptance 固定手順は単一ソースでなければならない

acceptance の固定手順は一箇所に集約されなければならない（MUST）。
固定手順を OpenCode コマンドテンプレート（例: `.opencode/commands/cflx-accept.md`）に置く場合、オーケストレーターは `{prompt}` に固定手順を含めず、可変コンテキストのみを渡さなければならない（MUST）。
acceptance の埋め込みシステムプロンプトは使用してはならず（MUST NOT）、固定手順はコマンドテンプレートからのみ供給される（MUST）。
acceptance_prompt_mode の `full` は互換エイリアスとして扱い、`context_only` と同じ挙動になる（MUST）。

Acceptance prompt construction MUST prepend exactly one selected acceptance skill prelude using `load skills: <accept_skill>`, where `<accept_skill>` is the effective configured acceptance skill and defaults to `cflx-accept` when omitted. Selecting a different acceptance skill MUST NOT duplicate the fixed acceptance procedure in the variable `{prompt}` payload.

#### Scenario: cflx-accept を使用する場合は context_only を採用する

- **GIVEN** acceptance_command が `/cflx-accept {change_id} {prompt}` を使用する
- **AND** the effective `accept_skill` is `cflx-accept`
- **WHEN** acceptance プロンプトを構築する
- **THEN** `{prompt}` は `load skills: cflx-accept` と change_id とパス、diff/履歴などの可変コンテキストのみを含む
- **AND** 固定の acceptance 手順は `.opencode/commands/cflx-accept.md` のみから供給される

#### Scenario: custom acceptance skill keeps variable context only

- **GIVEN** the effective `accept_skill` is `cflx-accept-with-speca`
- **WHEN** acceptance prompt construction builds the `{prompt}` payload
- **THEN** the payload contains `load skills: cflx-accept-with-speca`
- **AND** the payload still contains change metadata, paths, diff context, archive readiness context, previous acceptance output, user acceptance prompt, and history in the existing relative order
- **AND** the payload does not embed a second fixed acceptance checklist or a different verdict protocol

#### Scenario: full 指定でも固定手順は注入されない

- **GIVEN** acceptance_prompt_mode が `full` に設定されている
- **WHEN** acceptance プロンプトを構築する
- **THEN** 埋め込みの固定手順は注入されない
- **AND** `context_only` と同じ可変コンテキストのみが `{prompt}` に含まれる

## ADDED Requirements

### Requirement: Built-in SPECA acceptance skill

The orchestrator MUST include a built-in `cflx-accept-with-speca` skill that can be selected via `accept_skill`.

The `cflx-accept-with-speca` skill MUST preserve the Conflux acceptance verdict contract. It MUST produce exactly one final machine-readable acceptance verdict using the existing `pass`, `fail`, `continue`, or `gated` outcomes, with actionable `findings` for fail outcomes.

The skill SHOULD guide acceptance review to derive or select SPECA-style properties from OpenSpec deltas and changed files, perform a property-grounded proof attempt when tooling and context are available, and map blocking property failures into the existing acceptance verdict format.

The skill MUST NOT require changing `acceptance_command` merely to opt into SPECA-oriented acceptance behavior.

#### Scenario: cflx-accept-with-speca is available as a built-in skill

- **GIVEN** Conflux exposes its bundled skills to an agent runtime
- **WHEN** the built-in skill inventory is inspected
- **THEN** `cflx-accept-with-speca` is present
- **AND** `cflx-accept` remains present

#### Scenario: SPECA skill maps property failure to standard verdict

- **GIVEN** acceptance is using `accept_skill = "cflx-accept-with-speca"`
- **AND** a SPECA-style property proof attempt finds a blocking implementation mismatch with concrete repository evidence
- **WHEN** the acceptance reviewer returns a final verdict
- **THEN** the verdict uses the existing JSON `fail` outcome
- **AND** the `findings` array includes the property failure and concrete actionable evidence

#### Scenario: SPECA tooling unavailable does not create a new verdict protocol

- **GIVEN** acceptance is using `accept_skill = "cflx-accept-with-speca"`
- **AND** external SPECA tooling is unavailable in the agent environment
- **WHEN** the reviewer completes acceptance using available repository context
- **THEN** the reviewer still returns one of the existing Conflux acceptance verdicts
- **AND** it does not emit a SPECA-specific verdict format outside the Conflux acceptance contract
