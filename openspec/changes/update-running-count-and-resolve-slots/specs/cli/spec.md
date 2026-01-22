## MODIFIED Requirements

### Requirement: Running Mode Dashboard

TUI は Running モードでダッシュボード形式の UI を表示しなければならない（SHALL）。

#### Scenario: Display on processing completion
- **WHEN** すべての queued change が処理完了する
- **THEN** ヘッダーステータスが "Ready" に切り替わる
- **AND** ステータスパネルは進捗と経過時間のみを表示する
- **AND** `Ctrl+C` で終了できるよう表示を維持する

#### Scenario: Running mode header shows processing count
- **GIVEN** TUI が Running モードである
- **WHEN** 1 件以上の change が in-flight 状態（Applying/Accepting/Archiving/Resolving）である
- **THEN** ヘッダーは "Running <count>" を表示し、<count> は in-flight change の件数になる
- **AND** queued の change は <count> に含めない

#### Scenario: Status line uses selected change progress
- **GIVEN** TUI が任意のモードである
- **AND** 1 件以上の change が選択されている（x）
- **WHEN** ステータスパネルが描画される
- **THEN** 進捗バーは選択された change の total/completed を合算して反映する
- **AND** ステータス行は進捗バーと経過時間のみを表示する

#### Scenario: Status line shows accumulated running time
- **GIVEN** TUI が一度でも Running モードになっている
- **WHEN** Ready または Stopped モードでステータスパネルが描画される
- **THEN** 経過時間は累積の Running 時間を表示する

#### Scenario: Header hides status in stopped and error modes
- **GIVEN** TUI が Stopped または Error モードである
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーはステータスラベルを表示しない
