---
change_type: implementation
priority: high
dependencies:
  - compact-acceptance-retry-context
verifications:
  - id: finding-reconciliation-tests
    requirement: Runtime preserves completed findings during apply reconciliation and reopens them only from a new matching FAIL
    phase: pre-integration
    owner: conflux-acceptance
    trigger: acceptance-review
    automation: Cargo.toml
    evidence: cargo test task_parser && cargo test embedded_skills
    rerun: cargo test task_parser && cargo test embedded_skills
    prerequisites:
      - compact-acceptance-retry-context is implemented
references:
  - openspec/changes/compact-acceptance-retry-context
  - src/task_parser.rs
  - src/execution/apply.rs
  - skills/cflx-accept/SKILL.md
  - skills/cflx-apply/SKILL.md
---

# Change: acceptance finding reconciliationを堅牢化する

**Change Type**: implementation

## Problem/Context

Acceptance follow-upの照合がagent生成の説明文や任意のfinding codeに依存すると、applyが一部findingを修正して`[x]`へ変更した後、runtimeの再保証で`[ ]`へ戻り得る。緊急修正は全findingが完了した場合を保護するが、部分完了中の本文変更には恒久的な保証がない。

`compact-acceptance-retry-context`はsingle-section、normalized identity、latest FAILによるreopenを導入する。本changeは、そのidentityをagent出力だけに依存させず、apply中のreconciliationでcompleted状態を単調に保持し、accept/apply skillの競合する指示を解消する。

## Proposed Solution

Runtimeはfinding codeが存在する場合にそれを優先し、存在しない場合はfinding scope、rule kind、repository locationなどの正規化された構造情報からstable fallback identityを生成する。説明文やevidenceの変更だけではidentityを変えない。

Runtime-owned follow-upの`[x]`から`[ ]`への遷移は、新しいacceptance FAIL payloadが同じidentityを再報告した場合だけ許可する。Apply開始時、実行中、終了後の再保証はidentity単位のmergeとして動作し、agentが完了したfindingと別記したevidenceを保持する。

`cflx-apply`は通常taskのrefine規則からruntime-owned acceptance finding本文を除外し、本文不変・evidence別記を要求する。`cflx-accept`はstable code、atomic finding、current-worktree再検証、重複横断finding禁止を定義する。ただしstate correctnessはskill遵守ではなくruntimeが保証する。

## Acceptance Criteria

- Finding codeが欠落してもruntimeが同一repository defectへstable fallback identityを生成する。
- 一部findingだけが`[x]`の状態で説明文またはevidenceが変更されても、apply中の再保証は完了状態を保持する。
- `[x]`のfindingを`[ ]`へ戻せるのは、新しいacceptance FAILが同一identityを再報告した場合だけである。
- Serial/parallelの同等入力は同じidentityとcompletion transitionを生成する。
- `cflx-apply`はruntime-owned finding本文をrefineせず、verification evidenceを別記する。
- `cflx-accept`は1 findingを1原子的欠陥とし、実装欠陥と不足テストを分離し、包括的な重複findingを生成しない。
- 再acceptanceは現在のworktreeを再検証し、前回findingを新しいevidenceなしにstale再報告しない。

## Explicit Completion Conditions

- Task parserまたは共有finding normalization層に、code優先・構造fallbackのidentity生成が実装される。
- Apply開始時、実行中、終了後のfollow-up hydrationが同じidentity merge APIを使用する。
- Unit testsが部分完了、本文変更、code欠落、同一FAIL reopen、異なるfinding非reopen、serial/parallel parityを検証する。
- Embedded skill testsが`cflx-apply`のrefine例外と`cflx-accept`のatomic/current-state guidanceを検証する。
- `cargo test task_parser`、`cargo test embedded_skills`、`cargo fmt --check`、`cargo clippy --all-targets --all-features -- -D warnings`が成功する。

## Dependencies

`compact-acceptance-retry-context`が導入するsingle current section、normalized finding representation、latest FAIL update境界を前提とする。

## Out of Scope

- Acceptance retry回数、repeated-finding stall policy、human escalation threshold。
- Checkpoint/marker schema、restart routing、serial checkpointのchange-local分離。
- Acceptance checkboxの廃止、またはcheckpointを唯一のauthoritative work sourceへ変更すること。
- Prompt history compact化。これは依存changeが担当する。
