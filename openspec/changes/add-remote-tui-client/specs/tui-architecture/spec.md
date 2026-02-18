## ADDED Requirements
### Requirement: リモートデータソース対応
TUI は `--server` が指定された場合、ローカルの共有状態ではなくリモート API をデータソースとして使用しなければならない（MUST）。

#### Scenario: リモートデータソースに切り替わる
- **GIVEN** `--server` が指定されている
- **WHEN** TUI が change 一覧を構築する
- **THEN** ローカルの change 一覧を読み込まない
- **AND** リモート API の結果を使用する

### Requirement: プロジェクト単位のグルーピング表示
リモート接続時の change 一覧はプロジェクト単位でグルーピングして表示しなければならない（SHALL）。

#### Scenario: プロジェクトごとに表示が区切られる
- **GIVEN** サーバに 2 つのプロジェクトが登録されている
- **WHEN** TUI が change 一覧を表示する
- **THEN** change 一覧はプロジェクト単位で区切られて表示される

### Requirement: リモート更新の購読
TUI はリモートサーバの状態更新を購読し、既存の iteration 非後退ルールに従って反映しなければならない（MUST）。

#### Scenario: 古い iteration で上書きしない
- **GIVEN** TUI が `iteration_number=3` を表示している
- **WHEN** リモート更新で `iteration_number=2` が届く
- **THEN** TUI は `iteration_number=3` を保持する
