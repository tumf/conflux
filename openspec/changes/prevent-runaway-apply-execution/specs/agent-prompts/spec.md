## MODIFIED Requirements

### Requirement: Apply system prompt MUST enforce non-interactive iteration

Apply system prompt（`APPLY_SYSTEM_PROMPT`）は、agentがuserへ質問できず、task完了またはMaxIteration到達までoperational constraintsの下でautonomous decisionを行いながら作業を継続しなければならないことを明示しなければならない（MUST）。autonomyをunbounded verificationの許可として解釈してはならない（MUST NOT）。verification commandはdefaultで1回実行し、同一commandの再実行は1 Apply invocation内で最大3回までとし、各retryはrepository repairまたは具体的なenvironment recovery evidenceの後に限らなければならない（MUST）。no-change stability loopは禁止しなければならない（MUST）。bounded verificationが完了できないか不安定な場合、Applyはstructured `verification_timeout`または`verification_unstable` blocker factsを記録し、無限に継続せずConfluxへcontrolを返さなければならない（MUST）。

#### Scenario: Continue iteration without asking questions

- **GIVEN** apply executionがuncertain decision pointに遭遇する
- **WHEN** apply agentがtaskを処理する
- **THEN** Agentはuserへ質問しない
- **AND** Agentはrepository evidenceに基づく最善のautonomous decisionを行って続行する
- **AND** Agentはtask完了またはMaxIteration到達までiterationを継続する
- **AND** verification retryとinvocation runtime limitに従う

#### Scenario: Stability loop is prohibited

- **GIVEN** verification commandが1回完了した
- **AND** repository repairまたは具体的なenvironment recoveryが行われていない
- **WHEN** agentがstability証明だけを目的にcommand再実行を検討する
- **THEN** Apply guidanceは再実行を禁止する
- **AND** flakinessがtruthful completionを妨げる場合は`verification_unstable` factsを記録する

#### Scenario: Verification retry follows new evidence

- **GIVEN** verification commandが失敗した
- **AND** agentがrepository codeを修正したか具体的なenvironment recovery evidenceを取得した
- **WHEN** agentが同一verification commandを再実行する
- **THEN** 再実行は最大3回のtotal execution数に算入される
- **AND** limit到達後は4回目を開始せずblocker handoffを行う

### Requirement: Apply prompt MUST escalate implementation blockers

Apply guidanceはrepository-fixable work、mockable dependencies、non-repository external prerequisites、terminal rejection proposals、bounded verification failuresを区別しなければならない（MUST）。

Applyがrecoverable prerequisiteにより続行できない場合、`openspec/changes/{change_id}/tasks.md`へ`## Implementation Blocker #<n>`を追記しなければならない（MUST）。sectionはcategory、concrete file/log/command/output evidence、affected scope、prerequisiteまたはowner、verifiable unblock condition、next action、resumabilityを含み、checkboxを使用してはならない（MUST NOT）。Applyは同じfactsを持つ`IMPLEMENTATION_BLOCKER:` stdout blockを出力し、compatible machine-readable `BLOCKED` outcomeを返し、`REJECTED.md`を作成してはならない（MUST NOT）。

required foreground verificationがinvocation budget内に完了できない場合、guidanceはcategory `verification_timeout`を使用しなければならない（MUST）。evidence-bearing retry後もverificationがnondeterministicな場合、category `verification_unstable`を使用しなければならない（MUST）。両categoryは既存blocker schemaに加え、command、attempt count、duration、bounded output、repository diffまたはrecovery evidenceを保持しなければならず（MUST）、background verificationを残してはならない（MUST NOT）。

Apply guidanceはagentがfactsを報告し、Confluxがfactsをvalidateして最終`blocked`対`stalled` lifecycle classificationを所有することを明示しなければならない（MUST）。agentはproseまたはoutcome token spellingからcanonical lifecycle statusを主張してはならない（MUST NOT）。`REJECTED.md`はApplyがchange全体をrecoveryよりcloseすべき理由を明示的に確立した場合だけ許可される（MUST）。

#### Scenario: Apply records a recoverable prerequisite

- **GIVEN** Applyがrepository-only workとtest doubleでは現在のprerequisiteを満たせないと検証した
- **WHEN** blockerをescalateする
- **THEN** tasks.mdにcategory、evidence、affected scope、prerequisiteまたはowner、unblock condition、next action、resumabilityを持つ`## Implementation Blocker #<n>`が追加される
- **AND** sectionはcheckboxを含まない
- **AND** stdoutはmatching `IMPLEMENTATION_BLOCKER:` blockを含む
- **AND** Applyはcompatible machine-readable `BLOCKED` outcomeを出力する
- **AND** `REJECTED.md`を作成しない
- **AND** final lifecycle classificationをConfluxへ委ねる

#### Scenario: Apply does not externalize repository work

- **GIVEN** code、tests、specs、tasks、documentation、fixtures、mocks、stubsでfindingを解決できる
- **WHEN** Applyがescalation可否を評価する
- **THEN** repository workを継続するかrepository-fixable failureを報告する
- **AND** findingをexternal prerequisiteとしてlabelしない

#### Scenario: Apply distinguishes terminal rejection proposal

- **GIVEN** Applyがproposal premiseはinvalidまたはsupersededでchange全体をcloseすべきと確立する
- **WHEN** rejectionを提案する
- **THEN** stdoutはrejection proposalをrecoverable blocker outcomeと区別する
- **AND** worktree-local `REJECTED.md`はこのoutcomeだけに限定される

#### Scenario: Infrastructure verification blocker is not terminal rejection

- **GIVEN** ApplyまたはverificationがDocker unavailable、image-pull DNS timeout、package-registry timeout、port conflict、third-party outage、rate limiting等を観測する
- **AND** proposal premiseがinvalidまたはobsoleteである独立evidenceがない
- **WHEN** agentがblockerを記録する
- **THEN** guidanceはrecoverable structured blocker factsを記録させる
- **AND** `REJECTED.md`作成を指示しない

#### Scenario: Apply records bounded verification timeout

- **GIVEN** required foreground verificationがbounded Apply invocation内に完了できない
- **WHEN** repository-only repairでtimely evidenceを生成できない
- **THEN** tasks.mdにcategory `verification_timeout`と既存schema全fieldを持つnarrative Implementation Blockerが追加される
- **AND** stdoutにmatching structured blocker factsとcompatible `BLOCKED` outcomeが含まれる
- **AND** Applyはbackground verificationを残さずcontrolを返す
- **AND** final `blocked`または`stalled` classificationを主張しない
- **AND** `REJECTED.md`を作成しない

#### Scenario: Apply records unstable verification

- **GIVEN** 同一verificationがevidence-bearing retry limitへ到達した
- **AND** resultがnondeterministicなままである
- **WHEN** Applyがtaskをtruthfully completeできない
- **THEN** category `verification_unstable`として全attemptと既存schema全fieldを記録する
- **AND** compatible `BLOCKED` handoffを行う
- **AND** 追加stability loopを開始しない
- **AND** AcceptanceとConfluxがfactsからfinal lifecycle classificationを判断する
