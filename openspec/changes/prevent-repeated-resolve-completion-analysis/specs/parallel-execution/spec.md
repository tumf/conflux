## MODIFIED Requirements

### Requirement: Re-analysis triggers and non-blocking scheduler

re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了・reducer-visible queued intent reconciliation のいずれでもよい（MUST）。

利用可能スロットが 0 の場合でも、queued に ordinary dispatchable candidate が存在するなら、システムは queue classification、reducer-visible queued intent reconciliation、dependency analysis、operator-visible diagnostics を実行できなければならない（MUST）。ただし ordinary apply dispatch は、dispatch 直前に再計算した利用可能スロットが 1 以上になるまで開始してはならない（MUST NOT）。

resolve、workspace、または merge completion による即時 re-analysis trigger は、対応する completion event ごとに一度だけ消費されなければならない（MUST）。その trigger を一度評価した後、明示的な新しい completion event がない timer wake は、消費済み completion trigger を再利用して dependency analysis を開始してはならない（MUST NOT）。

スケジューラは reducer-visible queued work が存在するのに re-analysis または dispatch を開始しない場合、その理由を観測可能なログまたはイベントとして出力しなければならない（SHALL）。

TUI は distinct な re-analysis attempt を operator-visible に表示しなければならない（SHALL）。同じ attempt の重複 delivery は抑止してよいが、`remaining_changes` が同じという理由だけで別 attempt の analysis-started 表示を抑止してはならない（MUST NOT）。

<!-- Expected canonical result after archive: completion-triggered re-analysis remains immediate and capacity-safe, while each completion edge is consumed once so timer wakes cannot replay expensive analysis. -->

#### Scenario: completion triggerは一度だけ即時analysisを起動する

- **GIVEN** queued に ordinary dispatchable candidate が存在する
- **AND** resolve または merge completion event が発生する
- **WHEN** scheduler が completion 後の re-analysis を評価する
- **THEN** scheduler は debounce を待たずに dependency analysis を一度実行する
- **AND** ordinary apply dispatch は利用可能スロットに従う

#### Scenario: timer wakeは消費済みcompletion triggerを再利用しない

- **GIVEN** scheduler が completion event に対応する即時 dependency analysis を実行済みである
- **AND** queued set と利用可能スロットに変化がない
- **WHEN** 新しい completion event、queue addition、repair candidate、または slot recovery を伴わない timer wake が発生する
- **THEN** scheduler は消費済み completion trigger を理由に dependency analysis を再実行しない
- **AND** timer wake は既存の通常 debounce policy に従う

#### Scenario: 新しいcompletion eventは再び即時analysisを起動できる

- **GIVEN** 以前の completion trigger は消費済みである
- **AND** queued に未実行の change が存在する
- **WHEN** 別の resolve、workspace、または merge completion event が発生する
- **THEN** scheduler は新しい completion event に対して即時 re-analysis を実行できる
- **AND** capacity が回復していれば eligible な queued change を追加操作なしで dispatch する

#### Scenario: resolve中でもqueued candidateはanalysis対象になる

- **GIVEN** change A の resolve が進行中である
- **AND** resolve が現在の利用可能スロットを 0 にしている
- **AND** queued に ordinary dispatchable candidate change B が存在する
- **WHEN** 明示的な scheduler trigger により並列実行が re-analysis を評価する
- **THEN** scheduler は change B を analysis candidate として分類する
- **AND** dependency analysis を実行する
- **AND** change B の ordinary apply dispatch は開始しない
- **AND** dispatch が capacity gated である理由がログまたはイベントで観測できる
