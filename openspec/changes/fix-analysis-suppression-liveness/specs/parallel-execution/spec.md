## MODIFIED Requirements

### Requirement: Re-analysis triggers and non-blocking scheduler

re-analysis は apply/acceptance/archive/resolve の in-flight が存在していても開始できなければならない（MUST）。

re-analysis ループは dispatch の完了待ちでブロックされてはならない（MUST NOT）。

re-analysis の起動トリガは、キュー通知・デバウンスタイマー・in-flight 完了・reducer-visible queued intent reconciliation のいずれでもよい（MUST）。

利用可能スロットが 0 の場合でも、queued に ordinary dispatchable candidate が存在するなら、システムは queue classification、reducer-visible queued intent reconciliation、dependency analysis、operator-visible diagnostics を実行できなければならない（MUST）。ただし ordinary apply dispatch は、dispatch 直前に再計算した利用可能スロットが 1 以上になるまで開始してはならない（MUST NOT）。

resolve、workspace、merge completion、repair candidate addition、または slot recovery による即時 re-analysis trigger は、対応する state-transition event ごとに一度だけ利用されなければならない（MUST）。scheduler は queued work に対する re-analysis / dispatch evaluation をその trigger で実際に評価した後にのみ trigger を消費しなければならず（MUST）、evaluation を実行しなかった loop で未評価の trigger を破棄してはならない（MUST NOT）。

一度消費した completion、repair candidate、または slot recovery trigger は、明示的な新しい state-transition event がない timer wake で再利用されてはならない（MUST NOT）。timer wake は有限時間 queue debounce policy に従わなければならず（MUST）、debounce 経過後も同一の completed analysis input を変更なしに反復実行してはならない（MUST NOT）。

scheduler は ordinary timer-driven dependency analysis の直前に、queued change の analysis 入力、in-flight membership、利用可能 capacity、および repository-visible effective dependency-base evidence を表す deterministic runtime signature を評価しなければならない（MUST）。同じ signature に対する usable analysis result が active process 内ですでに完了している場合、明示的な新しい queue addition、completion、repair candidate、または slot recovery event を伴わない timer wake は高価な dependency analyzer を再実行してはならない（MUST NOT）。

signature は同一 change ID の proposal dependency、prompt-relevant metadata、または analyzer が読む proposal content の変更を識別できなければならず（MUST）、queued ID と件数だけで構成してはならない（MUST NOT）。queued と in-flight の双方について prompt が参照する proposal file content を含めなければならない（MUST）。effective dependency-base revision は dependency classification が merge evidence を評価する同じ selected branch/ref から解決されなければならず（MUST）、その ref revision が変化した場合は current checkout commit が不変でも signature は変化しなければならない（MUST）。

signature 構築に必要な proposal read または effective-base revision resolution が失敗した場合、scheduler は fail-open で dependency analysis を許可し、signature を記録せず、loop を終了してはならない（MUST）。ただし ordinary timer による再試行は既存の 10 秒 queue debounce cadence より頻繁に実行してはならず（MUST NOT）、失敗が継続する間の 500 ms timer wake ごとに proposal/VCS probe または dependency analyzer を起動してはならない（MUST NOT）。新しい明示的 edge trigger はこの失敗再試行 deadline を event ごとに一度 bypass してよい（MAY）。

queue addition、completion、repair candidate、および slot recovery の明示的 edge trigger は、matching signature が存在しても event ごとに一度の即時 analysis を許可しなければならない（MUST）。scheduler は analyzer result provenance を runtime 内で識別しなければならない（MUST）。healthy LLM result または意図的な metadata-only result は non-expiring completed signature を記録してよい。recoverable LLM failure による metadata fallback は degraded signature として記録し、記録から5分後の最初の eligible timer wake で unchanged input に対する一度の retry を許可しなければならない（MUST）。直前の repository probe deadline はこの degraded expiry を越えて retry を遅延させてはならない（MUST NOT）。

usable result を生成しない analyzer path は completed signature を記録してはならない（MUST NOT）。reducer-visible queued work が残る場合、その unusable result だけを理由に scheduler loop を終了してはならず（MUST NOT）、次の debounce-eligible timer evaluation または明示的 edge による retry を許可しなければならない（MUST）。

analysis 後も available capacity が正で、in-flight work が空であり、selected dispatch が 0 件である場合、scheduler はその result による suppression を記録してはならない（MUST NOT）。次の debounce-eligible timer evaluation は同じ input を再分析できなければならない（MUST）。

manual resolve、automatic resolve、workspace task、background merge、deferred retry、または failure / early-return path によって利用可能スロットが回復する場合、scheduler は explicit wake event、slot recovery detection、または現在 signature の変化を検出する有限時間 timer evaluation により queued work を再評価しなければならない（MUST）。sticky な過去 trigger または unchanged completed signature の反復利用だけを capacity-recovery liveness の根拠としてはならない（MUST NOT）。

analysis signature、completed record、および失敗再試行 deadline は active scheduler process の memory 内だけに保持しなければならず（MUST）、workflow next action、acceptance、archive、merge eligibility を決定する durable out-of-worktree state として保存または再利用してはならない（MUST NOT）。process restart 後の初回 eligible evaluation は以前の log、diagnostic cache、analysis signature、または retry deadline により抑止されてはならない（MUST NOT）。

スケジューラは reducer-visible queued work が存在するのに re-analysis または dispatch を開始しない場合、その理由を観測可能なログまたはイベントとして出力しなければならない（SHALL）。unchanged completed analysis input または bounded signature-failure retry による analyzer suppression は deduplicated operator-visible reason として識別可能でなければならない（SHALL）。

TUI は distinct な re-analysis attempt を operator-visible に表示しなければならない（SHALL）。同じ attempt の重複 delivery は抑止してよいが、`remaining_changes` が同じという理由だけで別 attempt の analysis-started 表示を抑止してはならない（MUST NOT）。analyzer invocation 前に suppression された timer evaluation は distinct analysis attempt として表示してはならない（MUST NOT）。

<!-- Expected canonical result after archive: ordinary timer suppression uses the same effective-base ref as dependency classification, unusable results cannot terminate queued work, and fail-open signature failures retry at a bounded cadence without restoring the 500 ms analyzer loop. -->

#### Scenario: effective dependency base ref change invalidates suppression

- **GIVEN** queued change と in-flight membership、capacity、proposal content、および current checkout commit は変化していない
- **AND** dependent change は selected effective dependency-base ref 上の integration evidence を待っている
- **WHEN** selected effective dependency-base ref の revision だけが前進する
- **THEN** current analysis input signature は previous completed signature と異なる
- **AND** scheduler は bounded timer evaluation により dependency analysis と dispatch eligibility を再評価する

#### Scenario: unusable empty analysis keeps queued scheduler alive

- **GIVEN** reducer-visible queued work が存在する
- **AND** in-flight work は空である
- **WHEN** dependency analyzer が usable order を生成せず終了する
- **THEN** scheduler は completed signature を記録しない
- **AND** scheduler loop はその result だけを理由に終了しない
- **AND** 次の debounce-eligible timer evaluation または明示的 edge は dependency analysis を再試行できる

#### Scenario: persistent signature failure is fail-open but rate-limited

- **GIVEN** proposal read または effective-base revision resolution が継続して失敗する
- **WHEN** ordinary 500 ms timer wake が10秒未満の間隔で繰り返される
- **THEN** scheduler は failed input をcompleted signatureとして記録しない
- **AND** scheduler loop は継続する
- **AND** ordinary timer による proposal/VCS probe と dependency analyzer invocation は10秒に一度を超えない
- **AND** deadline 後の最初の eligible evaluation は signature construction と dependency analysis を再試行する

#### Scenario: explicit edge bypasses signature failure retry deadline once

- **GIVEN** signature construction failure 後の ordinary retry deadline が未到達である
- **WHEN** new queue addition、completion、repair candidate、または slot recovery event が発生する
- **THEN** scheduler はその event に対して一度だけ即時 evaluation を許可する
- **AND** failure が継続する場合、event 消費後の ordinary timer wake は bounded retry cadence に戻る

#### Scenario: degraded expiry is not delayed by repository probe cadence

- **GIVEN** recoverable-failure metadata fallback の degraded signature が記録されている
- **AND** 5分expiry直前の repository probe が同じ signature を確認した
- **WHEN** degraded record の記録から5分が経過する
- **THEN** scheduler は最初の eligible timer wake で unchanged input に対する一度の retry を許可する
- **AND** 直前に設定された10秒 probe deadlineはretryをさらに遅延させない

#### Scenario: completed signature suppresses immediate timer I/O

- **GIVEN** scheduler は healthy usable analysis result と captured signature を記録した
- **WHEN** 記録から10秒未満の間に500 ms timer wakeが発生する
- **THEN** scheduler は dependency analyzerを再実行しない
- **AND** proposal fingerprintまたはVCS revisionを再取得しない
