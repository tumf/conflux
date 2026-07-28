## MODIFIED Requirements

### Requirement: Re-analysis triggers and non-blocking scheduler

re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了・reducer-visible queued intent reconciliation のいずれでもよい（MUST）。

利用可能スロットが 0 の場合でも、queued に ordinary dispatchable candidate が存在するなら、システムは queue classification、reducer-visible queued intent reconciliation、dependency analysis、operator-visible diagnostics を実行できなければならない（MUST）。ただし ordinary apply dispatch は、dispatch 直前に再計算した利用可能スロットが 1 以上になるまで開始してはならない（MUST NOT）。

resolve、workspace、merge completion、repair candidate addition、または slot recovery による即時 re-analysis trigger は、対応する state-transition event ごとに一度だけ利用されなければならない（MUST）。scheduler は queued work に対する re-analysis / dispatch evaluation をその trigger で実際に評価した後にのみ trigger を消費しなければならず（MUST）、evaluation を実行しなかった loop で未評価の trigger を破棄してはならない（MUST NOT）。

一度消費した completion、repair candidate、または slot recovery trigger は、明示的な新しい state-transition event がない timer wake で再利用されてはならない（MUST NOT）。timer wake は有限時間 queue debounce policy に従わなければならず（MUST）、debounce 経過後も同一の completed analysis input を変更なしに反復実行してはならない（MUST NOT）。

scheduler は ordinary timer-driven dependency analysis の直前に、queued change の analysis 入力、in-flight membership、利用可能 capacity、および repository-visible effective dependency-base evidence を表す deterministic runtime signature を評価しなければならない（MUST）。同じ signature に対する usable analysis result が active process 内ですでに完了している場合、明示的な新しい queue addition、completion、repair candidate、または slot recovery event を伴わない timer wake は高価な dependency analyzer を再実行してはならない（MUST NOT）。

signature は同一 change ID の proposal dependency、prompt-relevant metadata、または analyzer が読む proposal content の変更を識別できなければならず（MUST）、queued ID と件数だけで構成してはならない（MUST NOT）。effective dependency-base revision または同等の repository-visible integration generation が変化した場合も signature は変化しなければならない（MUST）。

queue addition、completion、repair candidate、および slot recovery の明示的 edge trigger は、matching signature が存在しても event ごとに一度の即時 analysis を許可しなければならない（MUST）。usable LLM result または既存の metadata-dependency fallback result が完了した場合、scheduler は analysis 開始前に取得した signature を completed input として記録しなければならない（MUST）。usable result を生成しない terminal analyzer path は completed signature を記録してはならない（MUST NOT）。

manual resolve、automatic resolve、workspace task、background merge、deferred retry、または failure / early-return path によって利用可能スロットが回復する場合、scheduler は explicit wake event、slot recovery detection、または現在 signature の変化を検出する有限時間 timer evaluation により queued work を再評価しなければならない（MUST）。sticky な過去 trigger または unchanged completed signature の反復利用だけを capacity-recovery liveness の根拠としてはならない（MUST NOT）。

analysis signature は active scheduler process の memory 内だけに保持しなければならず（MUST）、workflow next action、acceptance、archive、merge eligibility を決定する durable out-of-worktree state として保存または再利用してはならない（MUST NOT）。process restart 後の初回 eligible evaluation は以前の log、diagnostic cache、または analysis signature により抑止されてはならない（MUST NOT）。

スケジューラは reducer-visible queued work が存在するのに re-analysis または dispatch を開始しない場合、その理由を観測可能なログまたはイベントとして出力しなければならない（SHALL）。unchanged completed analysis input による analyzer suppression は deduplicated operator-visible reason として識別可能でなければならない（SHALL）。

TUI は distinct な re-analysis attempt を operator-visible に表示しなければならない（SHALL）。同じ attempt の重複 delivery は抑止してよいが、`remaining_changes` が同じという理由だけで別 attempt の analysis-started 表示を抑止してはならない（MUST NOT）。analyzer invocation 前に suppression された timer evaluation は distinct analysis attempt として表示してはならない（MUST NOT）。

<!-- Expected canonical result after archive: explicit scheduler edges remain immediate and one-shot, while ordinary timer wakes invoke dependency analysis only when the actual queued/in-flight/capacity/repository input differs from the last completed process-local input. -->

#### Scenario: unchanged timer inputはanalysisを反復しない

- **GIVEN** queued に ordinary dispatchable candidate が存在する
- **AND** dispatch capacity は 0 である
- **AND** scheduler は現在の analysis input signature に対する usable dependency analysis を完了済みである
- **WHEN** queue、in-flight、capacity、proposal input、effective dependency base、または明示的 edge event の変化なしに timer wake が繰り返される
- **THEN** dependency analyzer invocation count は増加しない
- **AND** queued work は保持される
- **AND** ordinary apply dispatch は開始されない
- **AND** suppression reason は deduplicated operator-visible evidenceとして観測できる

#### Scenario: debounce経過とanalysis時間はunchanged inputを再武装しない

- **GIVEN** 最後の queue change から debounce duration 以上が経過している
- **AND** dependency analysis の実行時間は debounce duration より短い、または長い
- **AND** analysis input signature は前回 completed signature と同じである
- **WHEN** ordinary timer evaluation が発生する
- **THEN** 経過時間または前回 analysis duration だけを理由として dependency analyzer は再実行されない

#### Scenario: explicit edgeはmatching signatureでも一度analysisする

- **GIVEN** scheduler は現在の analysis input signature を完了済みとして記録している
- **WHEN** queue addition、completion、repair candidate、または slot recovery の新しい event が発生する
- **THEN** scheduler は matching signature が存在しても event に対して一度 dependency analysis を実行する
- **AND** event 消費後の unchanged timer wake は同じ analysis を反復しない

#### Scenario: capacityまたはin-flight変化はtimer fallbackを再武装する

- **GIVEN** unchanged completed signature により ordinary timer analysis が抑止されている
- **WHEN** in-flight change が完了して利用可能 capacity または in-flight membership が変化する
- **THEN** current signature は previous completed signature と異なる
- **AND** scheduler は明示的 slot-recovery reason が保持されていない場合でも有限時間 timer evaluation により queued work を再分析できる
- **AND** eligible work は追加ユーザ操作なしで dispatch evaluation に到達する

#### Scenario: same-ID proposal changeはsignatureを無効化する

- **GIVEN** queued change ID の集合は変化していない
- **AND** scheduler は現在の proposal input を分析済みである
- **WHEN** queued change の dependency、prompt-relevant metadata、または analyzer-readable proposal content が変化する
- **THEN** analysis input signature は変化する
- **AND** 次の eligible evaluation は dependency analysis を実行する

#### Scenario: effective dependency base changeはsignatureを無効化する

- **GIVEN** queued change と in-flight ID の集合は変化していない
- **AND** dependent change は repository-visible integration evidence を待っている
- **WHEN** effective dependency-base revision または同等の integration generation が変化する
- **THEN** analysis input signature は変化する
- **AND** scheduler は updated repository evidence に対して dependency analysis と dispatch eligibility を再評価する

#### Scenario: metadata fallbackはunchanged failing inputの反復を止める

- **GIVEN** configured LLM analysis command は現在の input に対して recoverable failure を返す
- **AND** scheduler は metadata-dependency fallback から usable analysis result を得る
- **WHEN** state change または明示的 edge のない timer wake が繰り返される
- **THEN** scheduler は同じ failing LLM command を反復起動しない
- **AND** queue、capacity、proposal、repository evidence、または明示的 edge が変化すれば新しい attempt を実行できる

#### Scenario: process restartはruntime suppression stateを継承しない

- **GIVEN** previous process はある signature に対する analysis を完了していた
- **WHEN** scheduler process が再起動し同じ workspace state を評価する
- **THEN** previous process の analysis signature は workflow-control input として読み込まれない
- **AND** current process の初回 eligible dependency analysis は実行可能である
