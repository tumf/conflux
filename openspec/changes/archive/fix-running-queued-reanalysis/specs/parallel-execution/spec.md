## MODIFIED Requirements

### Requirement: Parallel Analysis Targeting

並列実行のanalysisはqueuedのchangeのみを対象にしなければならない（MUST）。

実行中のchangeが存在せず、queuedのchangeも空の場合、システムはオーケストレーションを終了しなければならない（MUST）。

analysis対象をqueuedに限定するため、queuedに含まれないchange（例: merged済みchange、実行済みchange、削除済みchange）はanalysis対象から除外されなければならない（MUST）。

queuedのchangeが空の場合、analysisを実行してはならない（MUST）。

re-analysis は完了イベントに依存せず、キュー変化やタイマーなどのトリガで起動可能でなければならない（MUST）。

re-analysis はメインの実行ループ進行に依存せず開始できなければならない（MUST）。

スロットが空いていない場合でも queued change の状態評価は継続できなければならず（MUST）、利用可能スロットが 0 から正の値へ戻った時点では queued change が存在する限り debounce 待ちを追加せずに再analysisを開始しなければならない（MUST）。

manual resolve の完了など、parallel slot を消費していた非 apply/archive タスクが終了して利用可能スロットが戻った場合も、queued change が存在すれば次のディスパッチ判定を行わなければならない（MUST）。

#### Scenario: queuedのみがanalysis対象になる
- **GIVEN** queuedにchangeが存在する
- **AND** queued以外に実行中のchangeが存在する
- **WHEN** 並列実行がanalysisを開始する
- **THEN** analysis対象はqueuedのchangeのみになる

#### Scenario: queued外のchangeはanalysis対象から除外される
- **GIVEN** queuedに含まれないchangeが存在する
- **AND** queuedには別のchangeが存在する
- **WHEN** 並列実行がanalysisを開始する
- **THEN** queued外のchangeはanalysis対象から除外される

#### Scenario: queuedが空ならanalysisを実行しない
- **GIVEN** queuedのchangeが存在しない
- **WHEN** 並列実行がanalysisを開始しようとする
- **THEN** analysisを実行しない

#### Scenario: 実行中とqueuedが空なら終了する
- **GIVEN** 実行中のchangeが存在しない
- **AND** queuedのchangeも空である
- **WHEN** 並列実行ループが次のanalysisを開始しようとする
- **THEN** analysisを実行しない
- **AND** オーケストレーションは完了状態になる

#### Scenario: キュー変化でre-analysisが起動する
- **GIVEN** 実行中のchangeが存在する
- **AND** queuedにchangeが追加される
- **WHEN** 並列実行がre-analysisを評価する
- **THEN** 完了イベントを待たずにre-analysisが開始される
- **AND** メインの実行ループ進行に依存しない

#### Scenario: スロット復帰時はdebounce待ちしない
- **GIVEN** queuedにchangeが存在する
- **AND** 利用可能なスロットが0であるため直前のディスパッチが保留されている
- **WHEN** 利用可能なスロットが正の値へ戻る
- **THEN** システムは追加の debounce 待ちなしで再analysisを開始する
- **AND** dispatch可能なchangeがあれば直ちに次のchangeを選択する

#### Scenario: manual resolve completion triggers dispatch reevaluation
- **GIVEN** manual resolve が parallel slot を1つ消費している
- **AND** 別のqueued changeが存在する
- **WHEN** manual resolve が完了して slot が解放される
- **THEN** システムは queued change に対する再analysis/dispatch判定を実行する
- **AND** apply/archive の完了イベントを別途待たない
