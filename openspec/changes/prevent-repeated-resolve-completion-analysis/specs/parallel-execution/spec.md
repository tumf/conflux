## MODIFIED Requirements

### Requirement: Re-analysis triggers and non-blocking scheduler

re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了・reducer-visible queued intent reconciliation のいずれでもよい（MUST）。

利用可能スロットが 0 の場合でも、queued に ordinary dispatchable candidate が存在するなら、システムは queue classification、reducer-visible queued intent reconciliation、dependency analysis、operator-visible diagnostics を実行できなければならない（MUST）。ただし ordinary apply dispatch は、dispatch 直前に再計算した利用可能スロットが 1 以上になるまで開始してはならない（MUST NOT）。

resolve、workspace、merge completion、repair candidate addition、または slot recovery による即時 re-analysis trigger は、対応する state-transition event ごとに一度だけ利用されなければならない（MUST）。scheduler は queued work に対する re-analysis / dispatch evaluation をその trigger で実際に評価した後にのみ trigger を消費しなければならず（MUST）、evaluation を実行しなかった loop で未評価の trigger を破棄してはならない（MUST NOT）。

一度消費した completion、repair candidate、または slot recovery trigger は、明示的な新しい state-transition event がない timer wake で再利用されてはならない（MUST NOT）。timer wake は既存の有限時間 queue debounce policy に従わなければならない（MUST）。

manual resolve、automatic resolve、workspace task、background merge、deferred retry、または failure / early-return path によって利用可能スロットが回復する場合、scheduler は explicit wake event、slot recovery detection、または有限時間 timer/debounce evaluation のいずれかにより queued work を再評価しなければならない（MUST）。sticky な過去 trigger の再利用だけを capacity-recovery liveness の根拠としてはならない（MUST NOT）。

スケジューラは reducer-visible queued work が存在するのに re-analysis または dispatch を開始しない場合、その理由を観測可能なログまたはイベントとして出力しなければならない（SHALL）。

TUI は distinct な re-analysis attempt を operator-visible に表示しなければならない（SHALL）。同じ attempt の重複 delivery は抑止してよいが、`remaining_changes` が同じという理由だけで別 attempt の analysis-started 表示を抑止してはならない（MUST NOT）。

<!-- Expected canonical result after archive: completion, repair-candidate, and slot-recovery edges remain immediate and capacity-safe, each edge is consumed only after an actual queued evaluation, timer wakes cannot replay it, and every capacity-release path retains bounded autonomous progress. -->

#### Scenario: completion triggerは一度だけ即時analysisを起動する

- **GIVEN** queued に ordinary dispatchable candidate が存在する
- **AND** resolve、workspace、または merge completion event が発生する
- **WHEN** scheduler が completion 後の re-analysis を評価する
- **THEN** scheduler は debounce を待たずに dependency analysis を一度実行する
- **AND** ordinary apply dispatch は利用可能スロットに従う

#### Scenario: timer wakeは消費済みedge triggerを再利用しない

- **GIVEN** scheduler が completion、repair candidate、または slot recovery event に対応する即時 dependency analysis を実行済みである
- **AND** queued set と利用可能スロットに変化がない
- **WHEN** 新しい completion event、queue addition、repair candidate、または slot recovery を伴わない timer wake が発生する
- **THEN** scheduler は消費済み edge trigger を理由に dependency analysis を再実行しない
- **AND** timer wake は既存の有限時間 debounce policy に従う

#### Scenario: evaluationされないedge triggerは早期消費されない

- **GIVEN** completion、repair candidate、または slot recovery event が発生する
- **AND** 現在の loop では queued work に対する re-analysis / dispatch evaluation が実行されない
- **WHEN** scheduler が次の wake または queued candidate ingestion を処理する
- **THEN** scheduler は未評価 trigger を評価前に消費したことを理由として必要な re-analysis 機会を失わない
- **AND** より新しい明示的 event が存在する場合は、その event の reason と既存優先規則に従う

#### Scenario: 新しいedge eventは即時analysisを再武装する

- **GIVEN** 以前の edge trigger は消費済みである
- **AND** queued に未実行の change が存在する
- **WHEN** 別の completion、repair candidate、または slot recovery event が発生する
- **THEN** scheduler は新しい event に対して即時 re-analysis を実行できる
- **AND** capacity が回復していれば eligible な queued change を追加操作なしで dispatch する

#### Scenario: capacity release pathはsticky triggerなしで進行する

- **GIVEN** ordinary apply dispatch capacity が manual resolve、automatic resolve、workspace task、background merge、または deferred retry により占有されている
- **AND** queued に eligible な change が存在する
- **WHEN** success、failure、deferred、または early-return path により capacity が回復する
- **THEN** scheduler は explicit wake event、slot recovery detection、または有限時間 timer/debounce evaluation により queued work を再評価する
- **AND** 過去の消費済み trigger を反復利用しなくても permanent starvation を起こさない

#### Scenario: repair candidateとslot recoveryは一度だけdebounceをbypassする

- **GIVEN** repair candidate addition または zero-to-positive slot recovery が発生する
- **AND** queued に analysis 対象が存在する
- **WHEN** scheduler がその state-transition edge を評価する
- **THEN** scheduler は debounce を待たずに一度 re-analysis を実行する
- **AND** 新しい同種 event のない後続 timer wake は同じ reason を再利用しない

#### Scenario: resolve中でもqueued candidateはanalysis対象になる

- **GIVEN** change A の resolve が進行中である
- **AND** resolve が現在の利用可能スロットを 0 にしている
- **AND** queued に ordinary dispatchable candidate change B が存在する
- **WHEN** 明示的な scheduler trigger により並列実行が re-analysis を評価する
- **THEN** scheduler は change B を analysis candidate として分類する
- **AND** dependency analysis を実行する
- **AND** change B の ordinary apply dispatch は開始しない
- **AND** dispatch が capacity gated である理由がログまたはイベントで観測できる
