## MODIFIED Requirements

### Requirement: Max Iterations Configuration

The configured `max_iterations` value SHALL be enforced by one per-change active-run cumulative Apply-dispatch counter shared by CLI, TUI, and remote-controlled worktree execution. A fresh process run SHALL reset the counter; no log or durable retry artifact SHALL restore it.

#### Scenario: Configure max iterations in config file

- **GIVEN** `.cflx.jsonc` contains `"max_iterations": 100`
- **WHEN** one change enters Apply through any executable frontend
- **THEN** that change starts no more than 100 configured Apply-agent dispatches during the active process run
- **AND** another change owns an independent total

### Requirement: Iteration Limit Finish Status

When one change exhausts its positive per-change Apply-dispatch budget, the sole budget owner SHALL return a typed `iteration_limit` outcome. CLI, TUI, and remote-controlled worktree run boundaries SHALL preserve that outcome and invoke existing finish-hook ownership exactly once.

#### Scenario: All frontends preserve typed iteration-limit ownership

- **GIVEN** the same per-change budget is exhausted through CLI, TUI, or remote-controlled execution
- **WHEN** the outcome crosses the respective run boundary
- **THEN** each boundary preserves `iteration_limit`
- **AND** no inner and outer owner invoke `on_finish` twice

### Requirement: Parallel Execution Configuration

Worktree execution SHALL be the only execution model. `max_concurrent_workspaces` SHALL configure its concurrency. `parallel_mode` is a retired key and SHALL NOT select runtime behavior.

#### Scenario: Retired parallel_mode key is rejected

- **WHEN** a loaded configuration contains `"parallel_mode"`
- **THEN** configuration loading fails before orchestration side effects
- **AND** the error instructs the operator to remove the retired key

#### Scenario: Configure max concurrent workspaces

- **WHEN** config contains `"max_concurrent_workspaces": 5`
- **THEN** at most 5 managed workspaces execute simultaneously
- **AND** CLI `--max-concurrent` overrides this value if provided

### Requirement: Parallel Configuration in Templates

The `init` command templates SHALL include worktree concurrency configuration and SHALL NOT emit the retired `parallel_mode` key.

#### Scenario: Claude template includes worktree options

- **WHEN** user runs `cflx init --template claude`
- **THEN** the generated config may include commented `max_concurrent_workspaces`
- **AND** it does not include `parallel_mode`

### Requirement: VCS Backend Configuration

設定ファイルで sole worktree execution path の VCS バックエンドを指定できなければならない（SHALL）。

#### Scenario: Configure VCS backend in config file

- **WHEN** `.cflx.jsonc` contains `"vcs_backend": "git"`
- **AND** executable orchestration starts
- **THEN** Git backend is used without an execution-mode flag

#### Scenario: VCS backend values

- **WHEN** `vcs_backend` is configured
- **THEN** valid values are `"auto"` and `"git"`
- **AND** the default is `"auto"`
