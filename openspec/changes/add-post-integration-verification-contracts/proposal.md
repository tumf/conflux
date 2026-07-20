---
change_type: implementation
priority: high
dependencies: []
verifications:
  - id: verification-contract-tests
    requirement: Typed parsing and strict validation enforce verification contracts
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: cargo test openspec::tests openspec_cmd
    rerun: cargo test openspec::tests openspec_cmd
    prerequisites: []
  - id: release-validation
    requirement: Installed skills preserve post-integration verification guidance
    phase: post-integration
    owner: repository-automation
    trigger: default-branch-integration
    automation: scripts/release.sh
    evidence: release CI logs
    rerun: rerun the release validation workflow
    prerequisites: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/proposal-metadata/spec.md
  - openspec/specs/cflx-proposal-validation/spec.md
  - openspec/specs/agent-prompts/spec.md
  - src/openspec.rs
  - src/openspec_cmd/validation.rs
  - skills/cflx-proposal/SKILL.md
  - skills/cflx-accept/SKILL.md
---

# Change: post-integration verification契約を追加する

**Change Type**: implementation

## Problem/Context

Confluxはapply、acceptance、archive、mergeの順でchangeを処理する。ところがproposalがremote default branchへのintegrationやdeployment後にしか観測できない条件をpre-integration completion conditionとして要求すると、acceptanceは未達をFAILとしてapplyへ戻し続け、同じchangeをmergeしなければ条件を満たせない循環が生じる。

実例ではGitHub Pagesがremote `main`へのpush後にだけ公開される一方、公開URLのHTTP 200とtitleがpre-merge acceptance条件に含まれた。自然言語のキーワードからverification phaseを推論する方式は、言い換え、多言語、fixture検証で誤検知と見逃しを生む。

## Proposed Solution

`proposal.md` frontmatterへ構造化された`verifications`宣言を追加する。各宣言は一意な`id`、対象requirement、`phase`、`owner`、`trigger`、repository-relative `automation` path、`evidence`、`rerun`、`prerequisites`を持つ。

`phase`は`pre-integration`または`post-integration`とする。pre-integration verificationはConflux acceptanceがrepository evidenceで完了を判定する。post-integration verificationはrepository automationがintegration/deployment後に実行し、Conflux acceptanceは未生成の外部結果を取得せず、automation、trigger、evidence publication、rerun手順、prerequisitesのrepository配線を検証する。

native strict validatorは宣言の型、必須field、phase/owner整合、一意性、automation pathの安全性と存在を決定的に検証する。自然言語によるphase推定はadvisory warningに限定し、workflow routingには使用しない。proposal authoringとacceptance skillを同じ契約へ更新する。

## Acceptance Criteria

- implementationまたはhybrid proposalは、実装完了を証明するpre-integration verificationを少なくとも1件宣言する。
- post-integration outcomeを持つproposalは、owner、trigger、tracked automation、evidence location、rerun action、prerequisitesを構造化宣言する。
- strict validationは不完全、不正、重複、unsafe path、存在しないautomationを決定的に拒否し、archive gateも同じfindingを返す。
- frontmatterまたは`verifications`を持たない既存spec-only proposalは引き続き有効である。
- acceptanceはpost-integration targetへHTTP/API accessせず、repository内automationの実装とfixture/local verification evidenceを評価する。
- post-integration automationの欠落や未配線はrepository-fixableなFAILとなり、未実行のoperational outcomeだけを理由にFAILへ戻さない。
- automation不能な外部prerequisiteは概要とnext actionを失わずstalled holdとなる。
- archive後もverification宣言はarchived `proposal.md`に保持され、Confluxは未確認のOperational Outcomeを達成済みと表示しない。

## Explicit Completion Conditions

- `src/openspec.rs`がtyped verification metadataと診断付きparse結果を提供し、既存のtolerant consumerを壊さない。
- `src/openspec_cmd/validation.rs`がstrict/archive-gate contractを実装し、validation中にnetwork accessしない。
- `skills/cflx-proposal/SKILL.md`がphase分類と構造化宣言をauthoring contractとして要求する。
- `skills/cflx-accept/SKILL.md`および互換acceptance skillがphase別rubricを共有する。
- parser、validator、archive preservation、skill embeddingのunit/integration testsが成功する。
- `cflx openspec validate add-post-integration-verification-contracts --strict --evidence warn`とarchive-equivalent validationが成功する。

## Dependencies

なし。このchangeは`bound-acceptance-retry-cycles`と並列実装できる。

## Out of Scope

- Confluxがremote default branch integration、deployment、公開URLを待ってterminal completionを判定する新lifecycle。
- 外部HTTP/GitHub状態をacceptance、archive、resume routingのauthoritative inputにすること。
- CI/CD provider固有のpost-deploy checker実装。
- `agent-exec` repositoryのPages workflow修正。
- Constitutionの変更。
