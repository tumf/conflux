## ADDED Requirements
### Requirement: server の resolve_command フラグは受け付けない
CLI は `cflx server --resolve-command` を受け付けてはならない（MUST NOT）。

#### Scenario: `--resolve-command` は不明なオプションとして扱われる
- **WHEN** ユーザーが `cflx server --resolve-command "true"` を実行する
- **THEN** CLI は不明なオプションとしてエラーを表示する
- **AND** 終了コードは非0である
