# Design: workflow split 後の prompt contract と ownership boundary を harden する

## Context

`split-workflow-skills-by-operation` は workflow 系 operation を dedicated skills へ分割し、acceptance では fixed procedure を command template、operation identity を dedicated skill、runtime context を Rust prompt builder が担う設計へ移行した。この分割自体は妥当だが、acceptance の実運用では verdict marker の markdown 変形が parser 契約から外れて `CONTINUE` fallback を引き起こした。また analyze / resolve では split 後に fixed guidance の duplicated source を後追いで是正した履歴があり、prompt ownership drift が複数 operation で再発し得ることが示されている。

## Goals

- acceptance verdict contract を prompt/template/spec/parser/tests の全層で整合させる
- workflow split 後の authoritative source 境界を operation ごとに再点検し、drift が再発しても即座に検出できるようにする
- legacy `cflx-workflow` compatibility router を維持しつつ、new orchestrator path の primary source を曖昧にしない

## Ownership Model

### Command-template-owned fixed guidance

- acceptance の fixed checklist / verdict workflow / output contract は `.opencode/commands/cflx-accept.md`

### Skill-owned fixed guidance

- analyze / apply / rejecting / cleanup-review / archive / resolve の fixed operation guidance は dedicated skills
- acceptance の dedicated skill `cflx-accept` は operation identity と scoped guidance を持つが、acceptance checklist や final output contract の primary source にはならない

### Rust-owned runtime context and enforcement

- Rust prompt builders は skill/template prelude と runtime-only context を注入する
- runtime parser / orchestration layer は documented marker contract を enforcement するが、undocumented formatting assumptionsに依存してはならない

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
