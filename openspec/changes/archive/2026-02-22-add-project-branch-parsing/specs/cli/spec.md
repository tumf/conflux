## ADDED Requirements

### Requirement: project add のブランチ表記を URL から解決する
CLI は `cflx project add` において、`https://github.com/<org>/<repo>/tree/<branch>` と `https://github.com/<org>/<repo>#<branch>` のブランチ表記を受け入れなければならない（SHALL）。
URL にブランチ表記が含まれる場合、CLI は `remote_url` をリポジトリのベース URL に正規化し、ブランチは抽出した値として扱わなければならない（MUST）。

#### Scenario: /tree/<branch> の URL を受け入れる
- **WHEN** ユーザーが `cflx project add https://github.com/org/repo/tree/develop` を実行する
- **THEN** CLI は `remote_url=https://github.com/org/repo` と `branch=develop` を使用する

#### Scenario: #<branch> の URL を受け入れる
- **WHEN** ユーザーが `cflx project add https://github.com/org/repo#develop` を実行する
- **THEN** CLI は `remote_url=https://github.com/org/repo` と `branch=develop` を使用する

### Requirement: project add のデフォルトブランチ解決
`cflx project add` でブランチが明示されない場合、CLI はリモートのデフォルトブランチを解決して使用しなければならない（MUST）。
明示的なブランチ引数が指定されている場合、URL 内のブランチ表記よりも引数の値を優先しなければならない（MUST）。

#### Scenario: ブランチ省略時にデフォルトブランチを使用する
- **GIVEN** リモートのデフォルトブランチが `main` である
- **WHEN** ユーザーが `cflx project add https://github.com/org/repo` を実行する
- **THEN** CLI は `branch=main` を使用する

#### Scenario: 引数指定が URL のブランチ表記を上書きする
- **WHEN** ユーザーが `cflx project add https://github.com/org/repo/tree/develop main` を実行する
- **THEN** CLI は `branch=main` を使用する
