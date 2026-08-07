## ADDED Requirements

### Requirement: AI command invocations have an absolute runtime limit

The common AI command runner MUST enforce `command_max_runtime_secs` as an absolute deadline measured from successful child spawn. The default MUST be 3,600 seconds, `0` MUST disable the deadline, and stdout or stderr activity MUST NOT extend it. Runtime-limit expiry MUST close retry admission for the invocation, terminate the owned process group through the existing graceful-then-forceful cleanup path, and return a typed non-retryable runtime-limit outcome.

#### Scenario: Continuous output does not extend the absolute deadline

**Given**: `command_max_runtime_secs` is enabled
**And**: an owned AI command emits output continuously
**When**: elapsed time from child spawn reaches the configured limit
**Then**: Conflux closes retry admission for the invocation
**And**: Conflux terminates and proves quiescence for the owned process group
**And**: the command is not automatically retried in the same run

#### Scenario: Zero disables the absolute deadline

**Given**: `command_max_runtime_secs` is `0`
**When**: an owned AI command remains active while satisfying all other lifecycle constraints
**Then**: Conflux does not terminate it solely because of total elapsed runtime
**And**: inactivity timeout and explicit cancellation remain independently enforceable

#### Scenario: Cleanup proof is required after runtime expiry

**Given**: an AI command exceeds its absolute runtime limit
**When**: bounded process-group cleanup cannot prove quiescence
**Then**: Conflux returns actionable cleanup diagnostics
**And**: it does not acknowledge successful termination
**And**: no later retry is admitted for that invocation

## MODIFIED Requirements

### Requirement: 無出力タイムアウトによる中断

コマンドキューは streaming 実行中に stdout/stderr の出力が一定時間発生しない場合、無出力タイムアウトとしてコマンドを中断しなければならない (MUST)。無出力タイムアウトは absolute runtime limit とは独立して評価され、出力行は無出力期限だけを延長し、absolute runtime deadline を延長してはならない (MUST NOT)。

#### Scenario: 無出力が続いた場合はタイムアウトで中断

- **GIVEN** 無出力タイムアウトが 900 秒に設定されている
- **AND** コマンドが stdout/stderr を一切出力しない
- **WHEN** 900 秒以上無出力が継続する
- **THEN** コマンドはタイムアウトとして中断される
- **AND** エラーメッセージに「inactivity timeout」が含まれる

#### Scenario: 出力があれば無出力タイムアウトだけが延長される

- **GIVEN** 無出力タイムアウトが 60 秒に設定されている
- **AND** absolute runtime limit が有効である
- **WHEN** コマンドが 30 秒ごとに stdout を出力する
- **THEN** 無出力タイムアウトは発生しない
- **AND** absolute runtime deadline は延長されない
