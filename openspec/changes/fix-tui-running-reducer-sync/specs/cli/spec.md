## MODIFIED Requirements

### Requirement: Running Mode Dashboard

TUI は Running モードでダッシュボード形式の UI を表示しなければならない（SHALL）。
正常完了時は Ready 表示に戻り、停止要求がない限り Stopped へ遷移してはならない。

TUI が shared reducer の display snapshot を `AppState` に同期する場合、Running mode の in-flight 状態を表す execution lifecycle events を reducer に反映してから display snapshot を適用しなければならない（MUST）。これにより、`ChangesRefreshed` 後も active display status と header count が stale reducer snapshot によって失われてはならない（MUST NOT）。

<!-- Expected canonical result after archive: Running Mode Dashboard will explicitly require reducer lifecycle-event sync before display snapshot application so active header counts survive refresh. -->

#### Scenario: Display on processing completion
- **WHEN** すべての queued change が処理完了する
- **THEN** ヘッダーステータスが "Ready" に切り替わる
- **AND** TUI は Select（Ready）モードに戻る
- **AND** ステータスパネルは進捗と経過時間のみを表示する
- **AND** `Ctrl+C` で終了できるよう表示を維持する

#### Scenario: Running mode header shows processing count
- **GIVEN** TUI が Running モードである
- **WHEN** 1 件以上の change が in-flight 状態（Applying/Accepting/Archiving/Resolving）である
- **THEN** ヘッダーは "Running <count>" を表示し、<count> は in-flight change の件数になる
- **AND** queued の change は <count> に含めない

#### Scenario: Reducer display sync preserves active header count
- **GIVEN** TUI が Running モードである
- **AND** shared reducer display snapshot が `AppState` の表示状態に同期される
- **WHEN** `ApplyStarted`, `AcceptanceStarted`, `ArchiveStarted`, or `ResolveStarted` が発生し、その後 `ChangesRefreshed` が発生する
- **THEN** 当該 change の表示状態は in-flight 状態として保持される
- **AND** ヘッダーは active change 数を `Running <count>` として表示し続ける
- **AND** queued のみの change は <count> に含めない

#### Scenario: Header hides status in stopped and error modes
- **GIVEN** TUI が Stopped または Error モードである
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーはステータスラベルを表示しない
