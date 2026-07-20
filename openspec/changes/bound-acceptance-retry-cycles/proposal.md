---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/orchestration-state/spec.md
  - src/acceptance.rs
  - src/history.rs
  - src/task_parser.rs
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/serial_run_service.rs
---

# Change: acceptance retry cycleを進捗ベースで制限する

**Change Type**: implementation

## Problem/Context

Acceptance FAILは現在、findingがapplyで解消可能か、前回からrepositoryに意味のある進捗があったかにかかわらずapplyへ戻る。parallel modeはhardcoded 10 apply+acceptance cyclesで停止するが、上限到達をterminal Errorとして扱う。serial path、CONTINUE ceiling、permission blockerには別々の規則があり、同じblockerの再試行と表示が一貫しない。

さらにruntimeはFAILごとに`## Acceptance #<n> Failure Follow-up`を`tasks.md`へ蓄積し、acceptance promptへ全attempt履歴、直前output、直前findingsを重複注入する。runtime自身のbookkeeping変更がrevisionを変えるため、commit hashだけでは実装進捗を判定できない。repo-local defectとexternal blockerが混在すると、修正対象とstalled理由も平坦化される。

## Proposed Solution

Acceptance findingを、少なくとも`scope`、stable `code`、optional repository `path`、human-readable `message`へ決定的に正規化する。既存string finding inputは後方互換で受理し、runtimeがstable identityを生成できる。

FAIL後は最低1回apply retryを許可する。次のacceptanceで同一finding identity集合が再発し、workspaceにsemantic progressがなければ、さらにapplyへ戻さずresumable stalledへ移す。semantic progressはsource、test、config、spec、またはruntime管理section外のtask変更とする。runtime管理follow-up、acceptance marker、attempt counter、外部logsだけの変更は進捗と数えない。

Runtimeは`tasks.md`へ単一の`Current Acceptance Follow-up`だけを所有・更新し、repository-fixable findingsのみcheckbox化する。external blockersはnon-checkbox metadataとして保持し、消失させない。Acceptance agentはread-onlyのままにする。

既存10-cycle ceilingは維持するが、exhaustionはterminal Errorではなくworkspace-local markerを持つresumable stalledにする。serial/parallelは共通判定を使い、prompt contextはcurrent diffとlatest normalized findingsを一度だけ含める。

## Acceptance Criteria

- 初回FAIL後はrepository-fixable findingをapplyへ1回戻す。
- 同一finding集合が再発しsemantic progressがなければ、次のapplyを開始せず`repeated_acceptance_findings` stalledとなる。
- findingが同じでもsource、test、config、spec、またはsubstantive task変更があればretryを継続できる。
- runtime-managed follow-upだけの変更はsemantic progressにならない。
- repo-local findingとexternal blockerが混在した場合、repo-local findingだけがcheckbox follow-upとなり、external blockerはnon-checkbox stalled metadataへ保持される。
- `tasks.md`にcurrent runtime follow-up sectionは最大1件となり、旧numbered sectionsはcompact化される。
- acceptance promptは全attempt履歴を注入せず、current diffとlatest findingsを重複なく渡す。
- apply+acceptance cycle 10到達はterminal Errorではなく`acceptance_cycle_limit_exhausted` stalledとなる。
- stalled evidenceはworkspace-local `APPLY_BLOCKED/marker.md`から再構成でき、外部state削除でnext actionが変わらない。
- explicit retryはacceptance-generated markerを安全にconsume/clearし、ordinary dispatchはmarkerを無視して再実行しない。
- serial/parallelで同一入力が同じretry/stalled分類になる。

## Explicit Completion Conditions

- finding normalization、identity comparison、mixed-scope classificationを共有helperとunit testsが担う。
- semantic progress snapshotがcommitted/uncommitted repository changesを扱い、runtime-owned regionsを除外する。
- `src/task_parser.rs`が単一current follow-upをupsertし、古いruntime follow-upを残さない。
- parallel dispatchとserial serviceが共通retry decisionを使用する。
- stalled markerがreason、retry count、finding identities、current findings、external blockers、resumability、next actionを保持する。
- process restart後にworkspace markerからstalled routingを復元するintegration testが成功する。
- `cflx openspec validate bound-acceptance-retry-cycles --strict --evidence warn`とarchive-equivalent validationが成功する。

## Dependencies

なし。このchangeは`add-post-integration-verification-contracts`と並列実装できる。後者が将来の循環proposalを防ぎ、本changeはruntime防御を提供する。

## Out of Scope

- apply+acceptance cycle ceiling値10の設定化。
- `acceptance_max_continues`のdefault値2/10不一致修正。
- Confluxによるremote deployment lifecycle。
- acceptance verdict protocolから`gated` compatibility tokenを除去すること。
- free-form findingの完全廃止。
- Constitutionの変更。
