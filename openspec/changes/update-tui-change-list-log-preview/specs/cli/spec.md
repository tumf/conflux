## MODIFIED Requirements
### Requirement: Log Entry Headers

TUI は analysis と resolve の operation ログに対して構造化ヘッダを表示し、追跡性を向上させなければならない (SHALL)。

#### Scenario: Analysis ログヘッダ形式
- **WHEN** analysis operation がログメッセージを出力する
- **THEN** ログエントリは `[analysis:N]` のヘッダで表示される
- **AND** N は analysis operation の iteration number を表す

#### Scenario: Resolve ログヘッダ形式
- **WHEN** resolve operation がログメッセージを出力する
- **THEN** ログエントリは `[resolve:N]` のヘッダで表示される
- **AND** N は resolve operation の iteration number を表す
- **AND** ヘッダには change_id が表示されない

#### Scenario: ログヘッダのカラーリングは一貫している
- **WHEN** ヘッダ付きログエントリが表示される
- **THEN** change_id が利用可能な場合、ヘッダは change_id hash に基づいた色分けで表示される
- **AND** 視認性のためヘッダは太字で表示される
