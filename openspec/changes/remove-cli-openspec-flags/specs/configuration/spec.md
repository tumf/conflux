## MODIFIED Requirements
### Requirement: OpenSpecコマンドの設定方法

OpenSpecコマンドはCLIフラグや環境変数で上書きできない（MUST NOT）。
オーケストレーターは設定ファイルのコマンドテンプレートを通じてOpenSpec/エージェントの実行方法を定義しなければならない（SHALL）。

#### Scenario: CLIフラグによるOpenSpecコマンド上書きが無効
- **WHEN** ユーザーが `cflx --help` を確認する
- **THEN** `--openspec-cmd` は表示されない
- **AND** CLIからOpenSpecコマンドを上書きできない
