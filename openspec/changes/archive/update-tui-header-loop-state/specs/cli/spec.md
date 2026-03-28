## MODIFIED Requirements

### Requirement: Running Mode Dashboard

TUI は Running モードでダッシュボード形式の UI を表示しなければならない（SHALL）。
正常完了時は Ready 表示に戻り、停止要求がない限り Stopped へ遷移してはならない。

ヘッダーステータスは個別 change の瞬間的 in-flight 有無ではなく、オーケストレーション全体ループ状態（`AppMode`）を主判定軸としなければならない（SHALL）。

- `AppMode::Select` では `Ready` を表示する。
- `AppMode::Running` では常に `Running` を表示し、in-flight change が 1 件以上ある場合に限り `Running <count>` 形式で件数を併記する。
- `AppMode::Stopping` では `Stopping` を表示する。
- `AppMode::Stopped` と `AppMode::Error` ではステータスラベルを表示しない。

`Running <count>` の `<count>` は in-flight change（Applying/Accepting/Archiving/Resolving）の件数のみを対象とし、queued は含めてはならない（MUST）。

#### Scenario: Display on processing completion
- **WHEN** すべての queued change が処理完了する
- **THEN** ヘッダーステータスが "Ready" に切り替わる
- **AND** TUI は Select（Ready）モードに戻る
- **AND** ステータスパネルは進捗と経過時間のみを表示する
- **AND** `Ctrl+C` で終了できるよう表示を維持する

#### Scenario: Running mode header shows processing count when in-flight exists
- **GIVEN** TUI が Running モードである
- **WHEN** 1 件以上の change が in-flight 状態（Applying/Accepting/Archiving/Resolving）である
- **THEN** ヘッダーは "Running <count>" を表示し、<count> は in-flight change の件数になる
- **AND** queued の change は <count> に含めない

#### Scenario: Running mode header remains Running with zero in-flight
- **GIVEN** TUI が Running モードである
- **AND** in-flight 状態の change が 0 件である
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーは "Running" を表示する
- **AND** ヘッダーは "Ready" を表示しない

#### Scenario: Select mode always shows Ready
- **GIVEN** TUI が Select モードである
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーは "Ready" を表示する

#### Scenario: Stopping mode header shows stopping
- **GIVEN** TUI が Stopping モードである
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーは "Stopping" を表示する

#### Scenario: Header hides status in stopped and error modes
- **GIVEN** TUI が Stopped または Error モードである
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーはステータスラベルを表示しない
