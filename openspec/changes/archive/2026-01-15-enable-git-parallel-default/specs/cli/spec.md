## MODIFIED Requirements
### Requirement: Parallel Execution Mode Flag
CLI は git worktree を使った parallel 実行を有効化する `--parallel` フラグを提供しなければならない（SHALL）。`--parallel` が指定されない場合は、設定ファイルの `parallel_mode` を優先し、未設定なら Git 検知に基づいて既定値を決定する。`--parallel` は設定より優先して parallel を強制する。

#### Scenario: `--parallel` で parallel を有効化
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** a `.git` directory exists
- **THEN** the orchestrator enters parallel execution mode
- **AND** changes are analyzed for parallelization opportunities

#### Scenario: 設定未指定でGitがある場合は既定でparallel
- **WHEN** user runs `openspec-orchestrator run` without `--parallel` flag
- **AND** config file does not contain `"parallel_mode"` key
- **AND** a `.git` directory exists
- **THEN** the orchestrator enters parallel execution mode
- **AND** changes are analyzed for parallelization opportunities

#### Scenario: 設定で無効化されている場合は逐次実行
- **WHEN** user runs `openspec-orchestrator run` without `--parallel` flag
- **AND** config file contains `"parallel_mode": false`
- **THEN** the orchestrator uses sequential execution mode
- **AND** no parallelization analysis is performed

#### Scenario: `--parallel` は設定より優先
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** config file contains `"parallel_mode": false`
- **AND** a `.git` directory exists
- **THEN** the orchestrator enters parallel execution mode

#### Scenario: Parallel mode requires git directory
- **WHEN** user runs `openspec-orchestrator run --parallel`
- **AND** no `.git` directory exists
- **THEN** the command exits with error code 1
- **AND** an error message indicates git repository is required for parallel mode

#### Scenario: Parallel mode with max concurrent limit
- **WHEN** user runs `openspec-orchestrator run --parallel --max-concurrent 4`
- **THEN** at most 4 workspaces are created simultaneously
- **AND** additional changes wait until a workspace becomes available
