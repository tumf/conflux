## MODIFIED Requirements

### Requirement: Dependent Change Skipping

失敗した変更に依存する変更は、失敗した依存先が解消されるまでdispatch対象から除外されなければならない（MUST）。accepted queue intentを持つ依存元changeはdependency-blocked queued workとして保持されなければならず（MUST）、scheduler-local queueからの削除とreducer reconciliationによる再追加を循環してはならない（MUST NOT）。

同一failed-blocker epochにおける依存元changeの再発見だけを理由として、新規queue addition、queue-edge bypass、distinct re-analysis attempt、重複`ChangeSkipped`、重複`DependencyBlocked`、または重複operator diagnosticを生成してはならない（MUST NOT）。この制約は、genuine dynamic queue addition、別changeまたはrepository evidenceによるanalysis signature変化、signature構築失敗後のbounded fail-open retry、degraded analysis resultの期限付きretryを抑止してはならない（MUST NOT）。

`RetryError(change-id)`がreducer stateを実際に変更してacceptedされた場合に限り、runtimeは対象IDを持つone-shot explicit-retry edgeをlive schedulerへ渡さなければならない（MUST）。schedulerはそのedgeをreconciliationおよびclassificationより前に一度だけ消費し、対象changeのephemeral failed classificationと関連blocker-notification epochを解除して一度再評価しなければならない（MUST）。refusedまたはno-op retry、通常の`AddToQueue`、generic `QueueNotification`はfailed classificationを解除してはならない（MUST NOT）。retry intentだけを依存成功の証拠として扱ってはならず（MUST NOT）、依存元changeのdispatch可否は通常のrepository and dependency evidenceから決定しなければならない（MUST）。

failed-dependent changeに対する`ChangeSkipped`はqueue intentの取消しではなく、failed dependencyによるdispatch exclusionの互換観測でなければならない（MUST）。authoritative blocked stateは`DependencyBlocked`で表されなければならない（MUST）。同一blocker fingerprintでは各eventを一度だけ発行し、blocker集合の変化、accepted retry後のrefailure、またはdequeue後のexplicit re-addは新しいepochとして一度の再発行を許可しなければならない（MUST）。

`RemoveFromQueue`または`DequeueChange`はblocked candidateをscheduler-local queueから除去し、そのnotification epochを解除しなければならない（MUST）。後続のexplicit re-addはgenuine queue additionとして扱われなければならない（MUST）。独立したqueued changeはfailed-dependent workの存在によって抑止されてはならない（MUST NOT）。

さらに、`MergeWait` により未統合のchangeを依存先に持つchangeは実行を保留し、今回のrunでは実行してはならない（MUST）。依存未解決により実行できないchangeはqueued状態のまま保持され、ステータス表示は依存待ちであることを示さなければならない（MUST）。

#### Scenario: Failed dependent remains stably queued

- **GIVEN** Aがfailedとして記録され、accepted queue intentを持つBがAに依存している
- **WHEN** schedulerがBを分類する
- **THEN** Bはdependency-blocked queued workとして保持される
- **AND** Bはapply dispatchされない
- **AND** local removalとreconciliation re-addの循環は発生しない

#### Scenario: Failed blocker transition emits bounded compatible events

- **GIVEN** Bが初めてfailed Aによるdependency-blockedへ遷移する
- **WHEN** blocked transitionが観測される
- **THEN** `ChangeSkipped(B,A)`と`DependencyBlocked(B)`は各一度発行される
- **AND** `ChangeSkipped`はBのaccepted queue intentまたはselectionを取り消さない

#### Scenario: Unchanged rediscovery creates no analysis edge

- **GIVEN** Bが同一failed-blocker epochで保持されている
- **WHEN** timer wakeとqueue reconciliationを複数回処理する
- **THEN** Bの再発見による`queued_added`は0である
- **AND** Bの再発見だけを理由とするanalyzer invocationまたは重複eventは発生しない

#### Scenario: Genuine new work still analyzes and dispatches

- **GIVEN** Bはfailed Aによりblockedである
- **WHEN** independent change Cがgenuinely addedされ実行capacityがある
- **THEN** Cのqueue edgeは通常どおりanalysisを起動できる
- **AND** Cはdispatch可能である
- **AND** Bはblocked queued workとして保持される

#### Scenario: Only accepted state-changing retry clears failure gate

- **GIVEN** AのfailureによりBがblockedである
- **WHEN** `RetryError(A)`がreducer stateを変更してacceptedされる
- **THEN** schedulerはtarget-ID-bearing one-shot retry edgeを受け取る
- **AND** reconciliation前にAのephemeral failed classificationと関連notification epochを解除する
- **AND** queued workを一度再評価する

#### Scenario: No-op and generic notifications do not clear failure

- **GIVEN** Aがfailedとして記録されている
- **WHEN** retryがrefusedまたはno-opである、通常の`AddToQueue`が発生する、またはgeneric queue notificationを受ける
- **THEN** Aのephemeral failed classificationは解除されない

#### Scenario: Retry does not prove dependency resolution

- **GIVEN** accepted retryによりAの過去failed markerが解除された
- **WHEN** Aがqueued、in-flight、unmerged、またはotherwise unresolvedである
- **THEN** Bはnormal dependency evidenceによりblockedのままである
- **WHEN** authoritative evidenceがAのresolutionを示す
- **THEN** Bは通常のdependency checksを通じてdispatch可能になる

#### Scenario: Refailure starts a new blocker epoch

- **GIVEN** Aへのaccepted retry後に以前のepochが解除された
- **WHEN** Aが再びfailed transitionを発生させる
- **THEN** Bは再度dependency-blockedになる
- **AND** 新しいepochのeventは各一度だけ発行される
- **AND** unchanged-state reanalysis loopは開始されない

#### Scenario: Dequeue and re-add are genuine state changes

- **GIVEN** Bがfailed Aによりblocked queuedである
- **WHEN** Bへの`RemoveFromQueue`または`DequeueChange`がacceptedされる
- **THEN** Bはscheduler-local queueから除去されnotification epochも解除される
- **WHEN** Bが後でexplicitly re-addedされる
- **THEN** その追加はgenuine queue additionとして一度評価される

#### Scenario: Blocked-only scheduler lifetime remains truthful

- **GIVEN** dispatchable workがなくfailed-dependent Bだけが残る
- **WHEN** scheduler lifetimeがfiniteである
- **THEN** schedulerはblockedまたはstalledとして終了し`AllCompleted`を発行しない
- **WHEN** scheduler lifetimeがpersistentである
- **THEN** schedulerはtimer pollingせずexplicit notificationを待つ

#### Scenario: Restart discards ephemeral failure tracking

- **GIVEN** process内でAとBのfailed-blocker epochが存在する
- **WHEN** processがrestartする
- **THEN** ephemeral failed trackerとnotification epochは空で開始する
- **AND** next actionはworkspace and Git evidenceから再計算される
