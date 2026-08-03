## MODIFIED Requirements

### Requirement: Dependent Change Skipping

失敗した変更に依存する変更は、失敗した依存先が解消されるまで実行対象から除外されなければならない（MUST）。その依存元 change に accepted queue intent がある場合、scheduler はそのchangeをdependency-blocked queued workとして保持しなければならず（MUST）、scheduler-local queueから削除してreducer reconciliationで再追加する循環を作ってはならない（MUST NOT）。同一のfailed dependency状態に対するtimer wakeまたはqueue reconciliationは、新規queue addition、distinct re-analysis attempt、重複`ChangeSkipped` event、または重複operator-visible skip diagnosticとして扱われてはならない（MUST NOT）。

accepted explicit retryまたはauthoritative success transitionが依存先のterminal failureを解除した場合、schedulerは対応するephemeral failed classificationを解除して一度再評価しなければならない（MUST）。retry intentだけを依存成功の証拠として扱ってはならず（MUST NOT）、依存元changeのdispatch可否は通常のrepositoryおよびdependency evidenceから決定しなければならない（MUST）。独立したqueued changeはfailed-dependent workの存在によって抑止されてはならない（MUST NOT）。

さらに、`MergeWait` により未統合の change を依存先に持つ変更は実行を保留し、今回の run では実行してはならない（MUST）。依存未解決により実行できない change は queued 状態のまま保持され、ステータス表示は依存待ちであることを示さなければならない（MUST）。

#### Scenario: Failed dependent remains stably queued

- **GIVEN** change Aがcurrent runでfailedとして記録されている
- **AND** accepted queue intentを持つchange BがAに依存している
- **WHEN** schedulerがBを分類する
- **THEN** Bはdependency-blocked queued workとして保持される
- **AND** Bはapply dispatchされない
- **AND** Bをscheduler-local queueから削除して次のreconciliationで再追加する循環は発生しない

#### Scenario: Unchanged failed dependency does not repeat analysis or skip output

- **GIVEN** Bがfailed Aへの同一dependency blockerで保持されている
- **WHEN** schedulerがtimer wakeとqueue reconciliationを複数回処理する
- **THEN** それらはBの新規queue additionとして扱われない
- **AND** distinct dependency analyzer invocationは追加されない
- **AND** 同一blockerの`ChangeSkipped` eventとoperator-visible skip diagnosticは反復しない

#### Scenario: Independent work continues

- **GIVEN** Bはfailed Aへの依存によりblockedである
- **AND** queued change CはAに依存していない
- **WHEN** schedulerに実行capacityがある
- **THEN** Cは通常のanalysisおよびdispatch対象となる
- **AND** Bはblocked queued workとして保持される

#### Scenario: Explicit retry reopens evaluation without proving success

- **GIVEN** AのfailureによりBがdependency-blockedである
- **WHEN** Aへのexplicit retryがacceptedされる
- **THEN** schedulerはAのephemeral failed classificationを解除する
- **AND** queued workを一度再評価する
- **AND** retry intentだけではAをresolvedとして扱わない
- **AND** Bはnormal repository and dependency evidenceがAのresolutionを示すまでdispatchされない

#### Scenario: Retried dependency resolves

- **GIVEN** Aへのexplicit retry後もBがqueuedである
- **WHEN** authoritative repository and completion evidenceがAのresolutionを示す
- **THEN** Bは通常のdependency checksを通じてdispatch可能になる

#### Scenario: Retried dependency fails again

- **GIVEN** Aへのexplicit retryによって以前のfailed classificationが解除された
- **WHEN** Aが再びfailed transitionを発生させる
- **THEN** Aは再度failedとして記録される
- **AND** Bは再度dependency-blockedになる
- **AND** 新しいblocker transitionの通知はboundedに発行される
- **AND** unchanged-state reanalysis loopは開始されない
