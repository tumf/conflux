---
change_type: implementation
priority: medium
dependencies:
  - bound-acceptance-retry-cycles
verifications:
  - id: acceptance-context-tests
    requirement: Runtime keeps one follow-up section and supplies latest-only acceptance context
    phase: pre-integration
    owner: conflux-acceptance
    trigger: acceptance-review
    automation: Cargo.toml
    evidence: cargo test task_parser && cargo test history && cargo test agent::prompt && cargo test parallel::dispatch && cargo test serial_run_service && cargo test embedded_skills
    rerun: cargo test task_parser && cargo test history && cargo test agent::prompt && cargo test parallel::dispatch && cargo test serial_run_service && cargo test embedded_skills
    prerequisites:
      - bound-acceptance-retry-cycles is implemented
references:
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/parallel-execution/spec.md
  - src/task_parser.rs
  - src/history.rs
  - src/parallel/executor.rs
  - src/serial_run_service.rs
  - skills/cflx-accept/SKILL.md
---

# Change: acceptance retry contextをcompact化する

**Change Type**: implementation

## Problem/Context

Runtimeはfollow-upを更新できるが、numbered headingと全finding checkboxを使い、repository findingとexternal blockerを区別しない。Acceptance promptはprevious findings、latest raw output、全attempt historyを重複注入する。Bundled acceptance skillはread-onlyへ移行済みだがcanonical guidanceがagentによる`tasks.md`編集を要求しており矛盾する。

## Proposed Solution

Runtime-owned follow-upを単一の`## Current Acceptance Follow-up`へ統一する。Repository-fixable findingだけをcheckbox化し、external blockerをnon-checkbox metadataとして保持する。Legacy numbered runtime sectionsは次回updateで置換する。

Acceptance promptはcurrent diffとlatest normalized findingsを一度だけ含める。Finalized FAIL payloadがある場合はraw outputと全attempt historyを重複注入しない。CONTINUEまたはcommand diagnosticsではbounded latest raw outputを許可する。Canonical/bundled guidanceをread-only runtime-owned contractへ統一する。

## Acceptance Criteria

- `tasks.md`のruntime-owned acceptance follow-upは最大1 sectionになる。
- 最新FAILで再報告されたrepository findingはstable identityが同じでもuncheckedへ戻り、obsolete findingは次回updateで除去される。
- Repository findingだけがcheckboxになり、external blockerはevidence/next action付きnon-checkbox metadataになる。
- Acceptance agentは`tasks.md`を編集せず、runtimeだけがfollow-upを書く。
- 3回以上のattempt後もpromptはcurrent diffとlatest findingsを一度だけ含み、旧attempt outputを含まない。
- CONTINUEまたはfinding-less command failureではlatest bounded diagnosticsを保持する。

## Explicit Completion Conditions

- Task parserがsingle-section upsert、legacy migration、identity-based completion、mixed-scope renderingを実装する。
- History/prompt builderがlatest-only contextを生成し、serial/parallelが同じbuilderを使う。
- Canonical specとbundled skillsからacceptance agentによるtask編集指示がなくなる。
- Unit testsがmigration、dedup、completion preservation、external metadata、3+ attempt prompt、diagnostic fallbackを検証する。

## Dependencies

`bound-acceptance-retry-cycles`のnormalized finding representationとscope classificationを使用する。

## Out of Scope

- Retry/stalled policyそのもの。
- Workspace marker schemaとexplicit retry consumption。
- Structured-finding-only protocolへの移行。
