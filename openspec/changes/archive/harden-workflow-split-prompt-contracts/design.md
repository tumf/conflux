# Design: workflow split 後の prompt contract と ownership boundary を harden する

## Context

`split-workflow-skills-by-operation` は workflow 系 operation を dedicated skills へ分割し、acceptance では fixed procedure を command template、operation identity を dedicated skill、runtime context を Rust prompt builder が担う設計へ移行した。この分割自体は妥当だが、acceptance の実運用では verdict marker の markdown 変形が parser 契約から外れて `CONTINUE` fallback を引き起こした。また analyze / resolve では split 後に fixed guidance の duplicated source を後追いで是正した履歴があり、prompt ownership drift が複数 operation で再発し得ることが示されている。

## Goals

- acceptance verdict contract を prompt/template/spec/parser/tests の全層で整合させる
- workflow split 後の authoritative source 境界を operation ごとに再点検し、drift が再発しても即座に検出できるようにする
- legacy `cflx-workflow` compatibility router を維持しつつ、new orchestrator path の primary source を曖昧にしない

## Ownership Model

### Per-operation ownership inventory

| Operation | Fixed Guidance Source | Rust Prompt Builder | Runtime Parser | Compatibility Note |
|-----------|---------------------|--------------------|--------------|--------------------|
| acceptance | `.opencode/commands/cflx-accept.md` (command template) | `build_acceptance_prompt_context_only` — injects change metadata, diff context, archive-readiness context, last output context, user prompt, history | `parse_acceptance_output` in `src/acceptance.rs` — enforces `ACCEPTANCE: PASS\|FAIL\|CONTINUE\|BLOCKED` standalone marker contract | `cflx-accept` skill provides operation identity + scoped guidance only; `cflx-workflow` Accept section provides legacy-equivalent guidance |
| apply | `cflx-apply` skill (dedicated skill) | `build_apply_prompt` — injects change_id, user prompt, history context, acceptance tail | N/A (no machine-readable output contract parsed by runtime) | `cflx-workflow` Apply section provides legacy-equivalent guidance |
| analyze | `cflx-analyze` skill (dedicated skill) | Rust injects candidate changes, progress context | N/A | `cflx-workflow` does not route analyze |
| archive | `cflx-archive` skill (dedicated skill) | `build_archive_prompt` — injects change_id, user prompt, history | N/A | `cflx-workflow` Archive section provides legacy-equivalent guidance |
| cleanup-review | `cflx-cleanup-review` skill (dedicated skill) | `build_cleanup_review_prompt` — injects change_id, paths, rules | `parse_cleanup_review_output` in `src/agent/prompt.rs` — enforces single `CLEANUP_REVIEW: CLEAN` standalone marker | `cflx-workflow` Cleanup Review section provides legacy-equivalent guidance |
| rejecting | `cflx-rejecting` skill (dedicated skill) | Rust injects change_id, REJECTED.md path | Runtime checks for `REJECTION_REVIEW: CONFIRM` or `REJECTION_REVIEW: RESUME` | `cflx-workflow` Rejecting Review section provides legacy-equivalent guidance |
| resolve | `cflx-resolve` skill (dedicated skill) | Rust injects conflict files, VCS state, merge plan, retry history | N/A | `cflx-workflow` does not route resolve |

### Intentional compatibility exceptions

- `cflx-workflow` remains a self-contained backward-compatible router: its operation sections intentionally duplicate a subset of fixed guidance from dedicated skills so that legacy prompts (`load skills: cflx-workflow`) can function without additional skill loads. This duplication is intentional and constrained — `cflx-workflow` MUST NOT become the authoritative source for new orchestrator prompts.
- `cflx-accept` skill supplements `.opencode/commands/cflx-accept.md` with verification-ownership checks and spec-only detection, but the fixed acceptance checklist and output contract remain command-template-owned.

### Command-template-owned fixed guidance

- acceptance の fixed checklist / verdict workflow / output contract は `.opencode/commands/cflx-accept.md`

### Skill-owned fixed guidance

- analyze / apply / rejecting / cleanup-review / archive / resolve の fixed operation guidance は dedicated skills
- acceptance の dedicated skill `cflx-accept` は operation identity と scoped guidance を持つが、acceptance checklist や final output contract の primary source にはならない

### Rust-owned runtime context and enforcement

- Rust prompt builders は skill/template prelude と runtime-only context を注入する
- runtime parser / orchestration layer は documented marker contract を enforcement するが、undocumented formatting assumptions に依存してはならない

## Contract Hardening Strategy

### 1. Make the acceptance marker grammar explicit

acceptance では final verdict marker に対して、少なくとも次を明文化する必要がある:

- canonical accepted form
- explicitly rejected markdown wrappers
- parser が tolerated input として受理する drift 範囲（もし許容するなら）
- tolerated input と canonical output の違い

### 2. Add boundary regression tests

各 operation について次をテスト戦略へ入れる:

- prompt builder が expected skill/template prelude を含む
- Rust prompt body が fixed guidance を authoritative に再定義しない
- output-contract-bearing operations は parser/runtime が documented marker contract と一致する
- embedded / installed skill assets が source-of-truth 境界を壊さない

### 3. Distinguish compatibility from authority

`cflx-workflow` は backward-compatible router として残るが、new orchestrator path の authoritative source と混同されないようにする。compatibility guidance は維持してよいが、new path の fixed instructions と差分が出る場合は intentional compatibility note として明示する。

## Risks

### Risk: acceptance parser を広げすぎて example block を誤認識する

Mitigation:
- parser tolerance を広げる場合も targeted cases に限定する
- code fence / example block 誤認識を防ぐ negative tests を追加する

### Risk: ownership audit が acceptance だけの局所修正で終わる

Mitigation:
- proposal scope に workflow split 対象 operation の ownership inventory と drift-detection tests を含める
- analyze / resolve の既存 follow-up と同じ boundary language を acceptance / other operations にも適用する
