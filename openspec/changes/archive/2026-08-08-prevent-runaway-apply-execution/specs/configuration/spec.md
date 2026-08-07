## ADDED Requirements

### Requirement: 絶対実行時間タイムアウト設定

オーケストレーターはJSONC設定ファイルでAI command invocation全体の絶対実行時間上限を設定できなければならない（MUST）。`command_max_runtime_secs`のdefaultは3,600秒とし、`0`はabsolute runtime limitを無効化しなければならない（MUST）。このkeyは他のtop-level optional config fieldsと同じmerge precedenceに従い、custom configがproject configをoverrideし、project configがglobal configをoverrideし、higher-precedence configがkeyを省略した場合はlower-precedence valueを保持しなければならない（MUST）。生成されるconfiguration exampleはkey、default、`0` disable semanticsを含まなければならない（MUST）。

#### Scenario: デフォルト設定でabsolute runtime limitが有効になる

- **WHEN** merged configurationに`command_max_runtime_secs`が存在しない
- **THEN** `command_max_runtime_secs`は3,600秒として扱われる

#### Scenario: absolute runtime limitを無効化する

- **GIVEN** `.cflx.jsonc`に以下の設定が存在する:
  ```jsonc
  {
    "command_max_runtime_secs": 0
  }
  ```
- **WHEN** AI command invocationが実行される
- **THEN** total elapsed runtimeだけを理由とするabsolute timeoutは適用されない
- **AND** inactivity timeoutとexplicit cancellationは独立して適用される

#### Scenario: custom configがprojectとglobalをoverrideする

- **GIVEN** global configの`command_max_runtime_secs`が3,600である
- **AND** project configの`command_max_runtime_secs`が1,800である
- **AND** custom configの`command_max_runtime_secs`が900である
- **WHEN** configuration mergeが完了する
- **THEN** effective valueは900である

#### Scenario: higher-precedence configが省略した値を保持する

- **GIVEN** global configの`command_max_runtime_secs`が1,200である
- **AND** project configとcustom configがこのkeyを省略する
- **WHEN** configuration mergeが完了する
- **THEN** effective valueは1,200である

#### Scenario: 生成configuration exampleにabsolute runtime keyを含む

- **WHEN** Confluxがdefault JSONC configuration exampleを生成する
- **THEN** exampleは`command_max_runtime_secs`を含む
- **AND** default 3,600秒と`0`による無効化を説明する
