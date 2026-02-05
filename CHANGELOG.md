# Changelog

All notable changes to this project will be documented in this file.

## [0.4.17] - 2026-02-05

### Documentation

- Update release documentation and workflow
- **openspec**: Add change for resolve pending MergeDeferred

### Miscellaneous

- Add cargo-release config and changelog
- Improve Makefile with cargo-release integration
- Update project version to 0.4.15 and enhance release workflow
- Update release configuration metadata in Cargo.toml
- Update project version and release configuration metadata

### Other

- Update-merge-deferred-resolve-pending
- Update-merge-deferred-resolve-pending (7/7 tasks, apply#1)
- Update-merge-deferred-resolve-pending

## [0.4.14] - 2026-02-05

### Documentation

- **openspec**: Add update-tui-resolve-queue change set

### Other

- Update-tui-resolve-queue
- Update-tui-resolve-queue
- 0.4.14

## [0.4.13] - 2026-02-04

### Documentation

- **openspec**: Add uncommitted change detection proposal

### Other

- Update-uncommitted-change-detection
- Update-uncommitted-change-detection
- 0.4.13

## [0.4.12] - 2026-02-04

### Other

- Pre-sync base into update-tui-iteration-guard
- 0.4.12

## [0.4.11] - 2026-02-04

### Documentation

- Document acceptance and resolve commands
- **openspec**: Add in-flight deps analysis proposal
- **openspec**: Add TUI iteration guard change draft

### Miscellaneous

- Add skill scaffolding and conflux skills
- Add cflx config and workflow skills

### Other

- Update-tui-iteration-guard
- Update-tui-iteration-guard
- Update-tui-iteration-guard
- Update-parallel-analysis-inflight-deps
- Update-parallel-analysis-inflight-deps
- 0.4.11

## [0.4.10] - 2026-02-01

### Other

- 0.4.10

## [0.4.9] - 2026-02-01

### Bug Fixes

- Remove unused imports to resolve clippy warnings
- Improve error handling when worktree deleted during archive
- **tui**: Stop toggling MergeWait/ResolveWait selection in stopped mode
- **tui**: Allow @ to toggle approval in MergeWait/ResolveWait without queue side effects
- **tui**: Mark resolve pending when resolve starts
- **tui**: Calculate actual occupied width for log preview display
- **tui**: Truncate log preview safely for Unicode
- Abort cancel monitoring task after apply completes

### Documentation

- Add refactoring proposals for loop decomposition
- Add proposal for config merge and removal of default commands
- Add openspec change proposals
- Add acceptance findings logging change proposal
- Propose cflx worktree default directory
- **openspec**: Add proposal for TUI error mode continuation
- **openspec**: Propose TUI iteration display update
- **openspec**: Propose preserving MergeWait on refresh
- **openspec**: Restore MergeWait and prioritize F5 resolve
- **openspec**: Allow toggling selection in MergeWait/ResolveWait
- Update TUI queue states and key bindings
- **openspec**: Align TUI +/D specs to Worktrees view
- **openspec**: Propose locking @/Space while active
- Translate tui-architecture spec to English
- **openspec**: Add worktree branch-exists recovery proposal
- Rename resolve wait to resolve pending
- Add openspec proposal for TUI change list log preview update
- **openspec**: Clarify relative timestamp format for TUI log preview
- **openspec**: Refine TUI log preview timestamp rules
- **openspec**: Propose acceptance sub-agent prompt parallelization
- **openspec**: Propose update TUI log view headers
- **openspec**: Propose update TUI change elapsed placement
- **openspec**: Propose update TUI log preview formatting
- **openspec**: Propose fix TUI log preview truncation
- **openspec**: Archive fix-tui-log-preview-truncation and publish English spec
- **openspec**: Add fix-tui-logs-wrap change draft
- **openspec**: Add refactor change drafts
- **openspec**: Add TUI change drafts
- **openspec**: Add acceptance prompt single-source draft
- **openspec**: Clarify resolve merge pre-commit cleanup

### Features

- Implement config merge and remove default command fallbacks

- Update config loading to merge-based system (platform < XDG < project < custom)
- Implement deep merge for HooksConfig
- Make command settings required (apply/archive/analyze/acceptance/resolve)
- Remove default command fallbacks (DEFAULT_*_COMMAND no longer used)
- Update all tests to work with new required command behavior
- Clean up backup files

Fixes all tasks in openspec/changes/update-config-merge-no-default-commands/tasks.md
- Implement TUI subcommand command logging

- Add command field to AcceptanceStarted/ArchiveStarted/ResolveStarted events
- Add operation tracker for serial mode to display correct operation labels
- Add ContextualOutputHandler wrapper for dynamic operation tracking
- Update TUI event handlers to display command strings consistently
- Add regression tests for event command propagation

Files changed:
- src/events.rs: Add command fields and tests
- src/parallel/executor.rs: Send command with AcceptanceStarted
- src/tui/state/events/completion.rs: Handle AcceptanceStarted command
- src/tui/orchestrator.rs: Add operation tracker
- src/orchestration/output.rs: Add ContextualOutputHandler
- src/serial_run_service.rs: Integrate operation tracking
- src/orchestrator.rs: CLI compatibility
- src/web/state.rs: Update event handling
- Add Acceptance #3 failure follow-up task
- TUI error mode transition and MergeWait operation consistency
- **tui**: Show last log preview in change rows
- **acceptance**: Support context-only prompt mode

### Miscellaneous

- Archive openspec change update-tui-merge-wait-execution-mark
- Add new file and update gitignore entries
- Refactor file structure in skills directory
- Remove outdated specification and proposal files
- Update command file structures
- Add Makefile with build, install, and version bump targets
- Add .tldrignore
- Update tasks.md - mark 9.1 and 9.2 as complete
- Add automatic git commit and tagging to version bump commands

### Other

- Refactor-parallel-scheduler-loop (apply#1)
- Refactor-parallel-scheduler-loop
- Refactor-orchestrator-run-loop (apply#1)
- Refactor-orchestrator-run-loop
- Ignore approved files
- Pre-sync base into refactor-orchestrator-run-loop
- Pre-sync base into refactor-parallel-scheduler-loop
- Refactor-tui-runner-handlers (apply#3)
- Refactor-tui-runner-handlers
- Refactor-archive-loop-helpers (apply#3)
- Refactor-archive-loop-helpers
- Refactor-tui-state-guards (apply#1)
- Refactor-tui-state-guards
- Pre-sync base into refactor-tui-state-guards
- Refactor-orchestrator-run-loop (apply#1)
- Refactor-orchestrator-run-loop
- Pre-sync base into refactor-orchestrator-run-loop
- Pre-sync base into refactor-archive-loop-helpers
- Pre-sync base into refactor-tui-runner-handlers
- Update-worktree-error-diagnostics (apply#2)
- Update-worktree-error-diagnostics
- Update-config-merge-no-default-commands (apply#1)
- Update-config-merge-no-default-commands (apply#2)
- Update-config-merge-no-default-commands
- **archive**: Update-worktree-error-diagnostics (attempt#1)
- **archive**: Update-worktree-error-diagnostics (attempt#2)
- **archive**: Update-worktree-error-diagnostics (attempt#1)
- Update acceptance #2 failure follow-up tasks
- **archive**: Update-worktree-error-diagnostics (attempt#2)
- Update-worktree-error-diagnostics (apply#1)
- Update-worktree-error-diagnostics
- Pre-sync base into update-config-merge-no-default-commands
- Update-acceptance-external-dependency-policy (apply#1)
- Update-acceptance-external-dependency-policy
- Refactor-tui-state-guards (apply#1)
- Refactor-tui-state-guards
- Pre-sync base into refactor-tui-state-guards
- Refactor-orchestrator-run-loop (apply#1)
- Refactor-orchestrator-run-loop (apply#1)
- Refactor-orchestrator-run-loop
- Update-tui-subcommand-command-logs (apply#2)
- Update-tui-subcommand-command-logs (apply#3)
- Update-tui-subcommand-command-logs (apply#4)
- Update-tui-subcommand-command-logs (apply#5)
- Update-tui-subcommand-command-logs
- Update-merge-archive-verification (apply#1)
- Update-merge-archive-verification (apply#2)
- Update-merge-archive-verification
- Refactor-parallel-scheduler-loop (apply#1)
- Refactor-parallel-scheduler-loop
- Refactor-archive-loop-helpers (apply#1)
- Refactor-archive-loop-helpers
- Pre-sync base into refactor-archive-loop-helpers
- Update-tui-merge-wait-refresh (apply#2)
- Update-tui-merge-wait-refresh
- Pre-sync base into update-tui-merge-wait-refresh
- Pre-sync base into refactor-parallel-scheduler-loop
- Pre-sync base into update-merge-archive-verification
- Pre-sync base into update-tui-subcommand-command-logs
- Update-tui-iteration-display (apply#1)
- Update-tui-iteration-display
- Pre-sync base into update-tui-iteration-display
- Update-tui-error-mode-continuation (apply#1)
- Update-tui-error-mode-continuation (apply#2)
- Update-tui-error-mode-continuation
- Update-tui-disable-marks-while-active (apply#1)
- Update-tui-disable-marks-while-active
- Update-acceptance-findings-logging (apply#1)
- Update-acceptance-findings-logging
- Pre-sync base into update-acceptance-findings-logging
- Pre-sync base into update-tui-disable-marks-while-active
- Pre-sync base into update-tui-error-mode-continuation
- Pre-sync base into refactor-orchestrator-run-loop
- Update-worktree-branch-exists-recovery (apply#1)
- Update-worktree-branch-exists-recovery
- Update-worktree-default-dir-cflx (apply#1)
- Update-worktree-default-dir-cflx
- Update-tui-specs-for-worktrees-plus-and-d (apply#1)
- Update-tui-specs-for-worktrees-plus-and-d
- Pre-sync base into update-tui-specs-for-worktrees-plus-and-d
- Pre-sync base into update-worktree-default-dir-cflx
- Pre-sync base into update-worktree-branch-exists-recovery
- **tui**: Remove log preview from change list rows
- Pre-sync base into update-worktree-error-diagnostics
- Update-tui-change-list-log-preview (apply#2)
- Update-tui-change-list-log-preview (apply#4)
- Update-tui-change-list-log-preview
- Establish guidelines for handling change proposals, apply operations, and acceptance reviews.

- Added new files `cflx-proposal.md`, `cflx-apply.md`, `cflx-accept.md`, and `cflx-archive.md`
- Defined requirements, guidelines, and policies for change proposal creation, implementation, acceptance, and archiving
- Provided detailed instructions for handling external dependencies, task management, verification, and documentation
- Emphasized the importance of proper task tracking, completion, and update throughout the change lifecycle
- Pre-sync base into update-tui-change-list-log-preview
- Update-acceptance-subagent-prompt (apply#2)
- Update-acceptance-subagent-prompt
- Update-tui-log-view-headers (apply#1)
- Update-tui-log-view-headers
- Update-tui-log-preview-formatting (apply#2)
- Update-tui-log-preview-formatting
- RunningモードChanges一覧の経過時間位置を更新
- Update-tui-change-elapsed-placement
- Pre-sync base into update-tui-change-elapsed-placement
- Pre-sync base into update-tui-log-preview-formatting
- Pre-sync base into update-tui-log-view-headers
- Pre-sync base into update-acceptance-subagent-prompt
- Remove unnecessary files related to log preview truncation.

- Remove log preview display requirement from TUI architecture spec
- Delete proposal.md and tasks.md from fix-tui-log-preview-truncation directory
- Fix-tui-logs-wrap (apply#6)
- Fix-tui-logs-wrap
- Pre-sync base into fix-tui-logs-wrap
- Fix-tui-logs-wrap (apply#1)
- Fix-tui-logs-wrap
- Refactor-command-queue-retry (apply#1)
- Refactor-command-queue-retry
- Pre-sync base into refactor-command-queue-retry
- 0.4.5
- Pre-sync base into fix-tui-logs-wrap
- Refactor-serial-run-service-flow (apply#8)
- Refactor-serial-run-service-flow (apply#2)
- Refactor-serial-run-service-flow
- Refactor-analyzer-streaming-parse (apply#1)
- Refactor-analyzer-streaming-parse (apply#3)
- Refactor-analyzer-streaming-parse (apply#4)
- Refactor-analyzer-streaming-parse
- Refactor-orchestrator-run-loop (apply#1)
- Refactor-orchestrator-run-loop
- Pre-sync base into refactor-orchestrator-run-loop
- 0.4.6
- Pre-sync base into refactor-analyzer-streaming-parse
- Refactor-tui-handler-deps (apply#2)
- Refactor-tui-handler-deps (13/14 tasks, apply#3)
- Refactor-tui-handler-deps (apply#4)
- Refactor-tui-handler-deps (apply#5)
- Refactor-tui-handler-deps (apply#6)
- Refactor-tui-handler-deps (apply#7)
- Refactor-tui-handler-deps
- Update-acceptance-prompt-single-source (apply#1)
- Update-acceptance-prompt-single-source
- Refactor-parallel-run-service-prep (apply#1)
- Refactor-parallel-run-service-prep (apply#3)
- Refactor-parallel-run-service-prep
- Update-tui-applying-progress-format (apply#1)
- Update-tui-applying-progress-format
- Pre-sync base into update-tui-applying-progress-format
- Pre-sync base into refactor-parallel-run-service-prep
- 0.4.7
- Pre-sync base into update-acceptance-prompt-single-source
- Update resolve merge to cleanup resurrected openspec/changes
- Update-resolve-merge-precommit-cleanup (apply#2)
- Update-resolve-merge-precommit-cleanup
- Fix-tui-ready-return (apply#1)
- Fix-tui-ready-return
- Pre-sync base into fix-tui-ready-return
- Pre-sync base into update-resolve-merge-precommit-cleanup
- 0.4.8
- Pre-sync base into refactor-tui-handler-deps
- Update Makefile for parallel index creation.

- Add new Makefile targets for creating fast and full indexes
- Update .PHONY rule to include new targets
- Pre-sync base into refactor-serial-run-service-flow
- 0.4.9

### Refactoring

- **tui**: Extract terminal suspend/restore pattern into helper functions
- **tui**: Extract worktree command execution into helper function
- **openspec**: Reorganize archive structure with date prefix
- Optimize database query performance for user authentication
- **tui**: Extract terminal and worktree helpers to dedicated modules
- **tui**: Remove unused ChangeState fields and methods
- Update access levels for improved modularity
- Optimize file handling for improved performance

### Styling

- Fix trailing whitespace and end-of-file issues

## [0.4.3] - 2026-01-29

### Bug Fixes

- Use interactive shell and inherit environment for agent commands
- Fix --parallel --dry-run
- **parallel**: Ensure correct base revision and archive in merge
- **workspace**: Initialize working copy after workspace creation
- Remove --ignore-working-copy from archive commits
- Resolve merge conflict in src/main.rs

Integrated changes from both branches:
- update-web-port-default: Auto-assigned port with conditional logging
- add-tui-qr-popup: QR code URL generation and passing to TUI

The resolution combines:
1. URL string generation for QR code feature
2. Conditional logging based on port value (0 = auto-assigned)
3. Proper variable naming (_web_handle) and URL return value
- Always create merge commits for individual changes
- Remove all --ignore-working-copy flags from jj commands
- Handle archived change resume flow
- Harden parallel merge and selection flow
- Show uncommitted badge only for queueable changes
- Resolve merge conflicts
- **config**: Remove mistaken archive completion retry settings
- **parallel**: Enforce resolve goals for git merges
- Auto-squash archive WIP commits
- Match archive WIP commits literally
- Correct git log flag typo (--fixed-string → --fixed-strings)
- **parallel**: Resolve infinite debounce loop on first iteration
- Execute AI agent commands in repository root with path context
- Implement immediate start for dynamic queues in parallel execution.
- Revamp Workspace Cleanup Guard Implementation
- Improve TUI Worktree Merge conditions and UX.
- Refine worktree merge conditions and TUI stability
- Refactor workspace cleanup guard in `src/parallel/cleanup.rs`
- **parallel**: Execute apply/archive commands in worktree directory
- **parallel**: Prepend cd to worktree in apply/archive commands
- Quote worktree path in parallel commands
- Verify archive in worktree before merge
- Improve tasks progress lookup
- Update spec format for archive - use NEW Requirements and translate to English
- Change spec header from NEW to ADDED Requirements
- Implement logic for fixing merged analysis loop
- Refine analysis loop to target queued changes only
- Ensure original branch set before parallel prep
- Fix slot availability count in parallel execution

- Create new design.md file with context, goals, decisions, risks, and migration plan sections
- Add modified requirements and scenarios for slot availability count in parallel execution in spec.md
- Update proposal to fix slot availability counting to exclude inactive states and specify affected specs and code files
- Improve concurrent dispatch for re-analysis loop
- Refactor re-analysis dispatch and tracking.
- Capture acceptance failure tail output
- Refactor order-based merge wait flow in parallel execution
- Improve error handling and TUI state management
- Surface parallel failures in TUI
- **tui**: Count active changes in header
- Fix parallel execution behavior in hooks and orchestrator loop

- Ensure `on_merged` hook is always called in successful merge paths in parallel mode
- Define requirements for `cli/spec.md` to manage acceptance, continuation, and archiving in the orchestrator loop
- Update log autoscroll feature to stop scrolling and clamp logs to oldest line when autoscroll is disabled
- Update acceptance testing scenarios and prompts
- Improve acceptance parsing and prompt to avoid code block false positives
- Fix serial mode acceptance to pass base branch

- Modified AgentRunner::run_acceptance_streaming signature to accept base_branch parameter
- Modified src/orchestration/acceptance.rs to get current branch and pass it to run_acceptance_streaming
- This ensures acceptance diff context is generated for serial mode (CLI/TUI) same as parallel mode
- All 883 tests pass
- 展開済みコマンドをTUI Logs Viewに表示
- Fix test-hook-simple: rename spec.md to proposal.md
- Update release workflow - fix rust-toolchain action and macOS runner
- Use platform-agnostic build steps for Windows compatibility
- Update Windows crate to fix CreateJobObjectW error
- Implement Send+Sync for JobObjectGuard on Windows

### Documentation

- Add OpenSpec change proposals for CLI improvements
- Update project documentation with new features and structure
- **tui**: Add proposal for dynamic key hints and approval state fix
- Add parallel executor refactoring design and specs
- Add refactor plan for codebase cleanup and deduplication
- Archive parallel dirty worktree change
- Add OpenSpec change proposals
- Add openspec change proposals
- Update openspec change drafts
- **openspec**: Archive changes and update specs
- Clarify handling of generated approved files
- **openspec**: Propose spec-test annotation checker
- **openspec**: Propose merge-wait on dirty base
- **openspec**: Refine progress-stall detector proposal
- **openspec**: Add TUI change proposals
- Add git parallel default proposal
- Add worktree and web monitoring proposals
- Add change proposal for parallel resolve status display
- Add change proposal for parallel merge completed status
- Add change proposal for tasks.md format guidance in AI agent prompts
- Add improve-workspace-resume-idempotency change proposal
- Add fix-tui-web-state-event-forwarding change proposal
- Add command execution queue proposal
- Document queue status merged workflow
- Add debounce scheduling change spec
- **openspec**: Add change proposal for parallel progress responsiveness
- Add proposal to remove hardcoded main branch references
- Revamp README and README.ja with enhanced features and instructions
- Update specs for crash recovery and streaming retry
- Add ai command runner change proposal
- Add TUI stop cancel change proposal
- Add change proposal for TUI worktree merge status labels
- Archive fix-worktree-merge-conditions and update code style guidelines
- Add change proposal for TUI log context headers
- Clarify apply prompt iteration policy
- Document execution loop refactor and worktree defaults
- Add change proposal for web UI title update
- Add mergewait resolve running change specs
- Update change list proposal filter documentation

- Add tasks for filtering proposal.md in the change list
- Specify conditions for including changes in the list based on the presence of `proposal.md`
- Define modified requirements for native task progress parsing in the `cli` specification
- Add parallel command queue fix proposal
- Update workspace resolution and execution design.docs

- Add new files `tasks.md`, `design.md`, `proposal.md`, and `spec.md` under `openspec/changes/update-apply-cwd-resolution`
- Define tasks, design, proposal, and specs for unifying workspace directory resolution and implementing parallel execution
- Specify impact on affected specs and code files in the proposal document
- Improve tasks completion handling across files
- Update parallel reanalysis order specifications
- Improve specifications for parallel analysis prompt
- Revamp project workflow and tech stack details
- Update project structure and filenames for consistency
- Update project documentation with workflow changes

- Update project.md with revised workflow steps and architecture patterns
- Add support for interactive and non-interactive modes
- Enhance debug output in render.rs to display offset values for better debugging and viewing experience
- Update validation process and flags in openspec documentation.
- **spec**: Update Running header count to include all active states
- Improve conflict resolution instructions
- Enhance project version control and management.
- **openspec**: Add change proposal for resolve wait status
- **openspec**: Add change proposal for workspace archive detection
- Update add-resolve-wait-status to leverage WorkspaceState::Archived
- Refactor documentation in AGENTS.md
- **openspec**: Draft mock-first external dependency policy

### Features

- Initial commit - OpenSpec Orchestrator
- Add configurable agent commands and interactive TUI dashboard
- **openspec**: Add agent command configuration and archive completed changes
- **hooks**: Add orchestrator stage hooks with configurable commands
- Complete test coverage analysis and improve orchestrator reliability
- **cli**: Add init subcommand and Ctrl+C quit support
- **config**: Restructure orchestrator config and archive completed changes
- Add comprehensive project improvements and documentation
- Add parallel change execution with jj workspaces
- Add comprehensive change management features
- Add max iterations config and release workflow proposals
- Implement release workflow and archive completed changes
- Archive completed changes and add new features
- Archive completed changes and add hook system redesign proposal
- **tui**: Remove Completed mode and simplify state transitions
- **parallel**: Add archive support with configurable command
- **tui**: Add parallel execution mode with conflict resolution
- **openspec**: Add parallel executor refactor specifications
- **ui**: Add elapsed time tracking and display for changes and orchestration
- Add error.log to .gitignore
- **refactor**: Add approved refactoring proposals for code organization
- **tui**: Add QR code popup for web UI access and auto port assignment
- **tui**: Add QR code popup for web UI access
- **events**: Unify event system across serial and parallel modes
- Add-web-approval-api
- **openspec**: Approve child process cleanup and merge commit fixes
- Add reliable cross-platform child process cleanup
- **tui**: Add elapsed time tracking for parallel execution
- Drop jj backend for parallel execution
- Warn on dirty worktree in parallel mode
- Add change
- Add
- **parallel**: Run git merge resolver with verification
- **parallel**: Delegate git merge to resolve_command
- **parallel**: Finalize archive commits via resolve_command
- **archive**: Add completion check retry config
- **parallel**: Enforce presync merge before integration
- Add loop history context to archive and resolve operations
- Add tasks.md format guidance to AI agent prompts
- **openspec**: Add TUI editor keybind proposal for direct file opening
- **tui**: Open proposal.md directly when pressing 'e' key
- **parallel**: Read progress from uncommitted worktree tasks.md
- **openspec**: Add worktree view and merge proposal
- Refactor TUI logic for worktree deletion and approval.
- Add agent crash recovery and streaming retry support
- Implement operation iteration in TUI log headers.
- Implement global resolve lock feature for parallel execution.

- Implement global resolve lock using `std::sync::OnceLock` in `tokio::sync::Mutex`
- Update `src/parallel/mod.rs` to include the global lock and remove instance lock
- Refactor `attempt_merge()` to utilize the global lock for serialization
- Add tasks for implementation, including creating `GLOBAL_MERGE_LOCK` and `global_merge_lock()` accessor function in `src/parallel/mod.rs`
- Implement retry logic for streaming execution.

- Implement retry logic in streaming execution for better error handling
- Add observability spec file for syncing TUI logs to debug file
- Define LogLevel enum with Info, Success, Warn, Error levels for logging behavior
- Implement REST API requirements for web dashboard refresh fix.
- **web**: Refresh change progress from worktrees
- Update default worktree base directory and configurations
- Enhance update prompts and apply system enforcement
- Improve progress event handling in executor.
- Update TUI task progress retention logic
- Update resolve prompt to disallow --no-verify option
- Implement automated setup script execution on worktree creation

- Define requirements for worktree setup script execution
- Propose automatic execution of setup script after creating a worktree
- Enforce prohibition of `--no-verify` in apply prompt for system prompts
- Enhance archive merge guards and validations.
- Implement Reliable Archive Tracking for Changes Directory in CLI
- Update log headers with iteration in TUI architecture
- Refactor command line interface specifications
- Implement conflict detection using `git merge-tree` output.

- Add new files and update existing ones to implement conflict detection using `git merge-tree` output analysis
- Update TUI workflow with new conflict detection results
- Validate changes with `npx @fission-ai/openspec@latest` and `cargo test` for related unit tests
- Add change
- Update archive state handling and task completion gates in openspec.
- Implement logging for TUI Worktrees view Enter operations
- Implement worktree guard for parallel apply execution

- Update requirements for parallel execution in `spec.md`
- Add tasks for implementation and validation in `tasks.md`
- Propose worktree guard for parallel apply in `proposal.md`
- Document design decisions for worktree guard in `design.md`
- Improve error handling and logging in Git workflow.
- Refine archive state detection and task checks
- Update TUI with merged progress refresh capability
- Update parallel reanalysis order specifications.
- Implement fix for merge resolve status
- Implement UTC-based build number generation.

- Added proposal, design document, tasks, and specifications for introducing UTC-based build number display
- Updated CLI and TUI version displays to include build number
- Addressed potential risk of duplicate identifiers with multiple builds within the same second
- Implement order-based logic for parallel reanalysis

- Create new design document for audit-parallel-reanalysis-gaps
- Define tasks for investigation, implementation, and validation
- Specify behavior for reanalysis during parallel execution and dependency handling
- Implement acceptance loop for confirmation before archive.
- Improve acceptance prompt functionality
- Implement acceptance CONTINUE state handling across CLI.
- Implement fix for parallel queue reanalysis.
- Refactor web UI for improved status alignment and summary
- Implement TUI acceptance status integration in architecture

- Add specifications, tasks, and design for TUI architecture to display 'accepting' status during execution
- Document TUI acceptance status to differentiate acceptance processing
- Include events for TUI status update during acceptance execution
- Verify returning to previous status after acceptance completion
- Implement acceptance failure handling with apply retry loop
- Update resume acceptance process in openspec
- Implement change delta conflict check feature.

- Implemented conflict detection logic for spec delta changes
- Added CLI subcommand for conflict detection
- Ran necessary tests and formatting checks
- Update dependency analysis criteria documentation.
- Update acceptance behavior to default CONTINUE
- Normalize acceptance markers and tighten dependency analysis
- Implement order-based slot launch for parallel execution

- Implement and validate order-based slot launch in the system
- Introduce new specification document for parallel execution requirements
- Address queue changes, slot driving reanalysis, and monitoring during parallel execution
- Implement stall merge timeout circuit breaker in multiple modes.

- Added new files and specifications related to handling merge stall timeouts
- Defined requirements for detecting and handling merge stall events in CLI, TUI, web monitoring, and parallel execution scenarios
- Implemented customizable configuration settings for merge stall detection and circuit breaker thresholds
- Enhance log analysis and resolve tracking
- Enhance acceptance logs with iteration tracking.
- Add OpenAPI documentation for web API and WebSocket endpoints
- Update TUI exit shortcut functionality and behavior
- Implement automatic clearing of MergeWait status
- Update web UI change status across multiple files
- Implement parallel re-analysis dispatch feature
- Improve archive tasks fallback mechanism
- Improve worktree preservation during force stops.
- Refactor re-analysis scheduler for non-blocking execution
- Implement web UI execution controls for remote operations

- Implement web UI execution controls for remote operations
- Include server-side control API for web execution controls
- Update specifications for web monitoring and CLI
- Add new documentation files for design and specifications
- Enhance merge-wait-resolve flow documentation
- Update worktree cleanup policy documentation and design
- Implement spinner display during accepting in TUI

- Add specifications for spinner animation display during item acceptance in TUI
- Justify the visual clarity improvement with spinner display during accepting status
- Define tasks for implementing and testing accepting spinner features
- Enhance merge conflict resolution and log auto-scroll logic
- Update OpenSpec to remove 'completed' status across files
- Implement proposed changes for fixing TUI accepting stop status

- Implement proposed changes to fix the issue with TUI accepting status remaining displayed after forced stop
- Add specifications for handling interrupted changes in the CLI
- Include tasks for implementation and validation in the documentation
- Update status labels display across UI and TUI.
- Update worktree deletion process with branch management improvements
- Improve verification strategy and output format
- Update dependency-blocked change status across specs and tasks
- Update config loading order to prefer XDG paths
- Update change ID handling in orchestrator module
- Add on_merged hook change proposal
- Improve order of TUI event forwarding in Parallel mode
- **openspec**: Add change proposal for task parser excluded sections checkbox handling
- Update acceptance failure follow-up authoring specifications
- Add acceptance criteria for git clean check.

- Implemented acceptance criteria for git clean check
- Added requirements for acceptance to fail when git working tree is dirty
- Updated code files to include git status clean check functionality
- **openspec**: Add acceptance tail to apply change proposal
- Add acceptance tail to apply prompt

- Add last_acceptance_output block to apply prompt from AcceptanceHistory
- Inject tail only on first apply attempt after acceptance failure
- Add tests for acceptance tail injection and priority
- Update tasks.md to reflect completion
- **openspec**: Add change proposal for TUI command logs display
- Implement acceptance base diff feature across specs and tasks.
- Add test change for on_merged hook parallel test
- Add test change v2 for on_merged hook parallel test
- Add test change for on_merged hook testing
- Add test-hook-simple change for on_merged hook testing
- Add completed task to test-hook-simple

### Miscellaneous

- Add pre-commit hooks and update documentation formatting
- **opencode**: Add worktree command config
- **openspec**: Remove duplicated add-tui-resolving-status draft
- Update project version number to 0.2.0
- Update setup script to install pre-commit hooks based on availability
- Implement worktree pre-commit hook generator in `.wt/setup`
- Improve project setup and directory handling
- Optimize cache handling in worktree script
- Remove backup files from apply commit
- Remove outdated TUI documentation.
- Remove outdated test files and compatibility tests
- Update project version to 0.4.0
- Update gitignore to include `.leann/` directory
- Refactor documentation structure in `openspec/changes` directory
- Update GitHub workflow for Conflux installation configuration
- Bump version to 0.4.3

### Other

- Redesign-hook-system
- Redesign-hook-system
- Add-hardcoded-apply-system-prompt
- Refactor-tui-module-structure
- Refactor-tui-module-structure
- Refactor-tui-module-structure
- Refactor-tui-module-structure
- Refactor-tui-module-structure
- Remove-completed-mode
- Remove-completed-mode
- Improve-dynamic-key-hints
- Add-jj-parallel-apply
- Add-jj-parallel-apply
- Add-jj-parallel-apply
- Refactor-parallel-executor
- Refactor-parallel-executor
- Refactor-parallel-executor
- Refactor-parallel-executor
- Refactor-codebase-cleanup
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Add-git-worktree-parallel
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Refactor-config-module
- Refactor-tui-state
- Refactor-vcs-abstraction
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Sync-readme-translations
- Fix-cli-spec-language
- Update-spec-purpose-fields
- Remove-dummy-spec
- Fix-jj-conflict-detection
- Fix-jj-conflict-detection
- Fix-archive-failed-status
- Refactor-parallel-executor
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Update-agents-project-structure
- Clear-new-badge-on-interaction
- Refactor-tui-key-hints-layout
- Grayout-archived-checkbox
- Fix-stopped-mode-approval-queue
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Fix-stopped-mode-approval-queue
- Add-missing-spec-tests
- Add-propose-command
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Add-propose-command
- Fix-stopped-mode-approval-queue
- Add-workspace-resume
- Add-missing-spec-tests
- Skip-dependent-changes-on-error
- Skip-dependent-changes-on-error
- Skip-dependent-changes-on-error
- Preserve-workspace-on-error
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Preserve-workspace-on-error
- Fix-dynamic-queue-removal
- Add-periodic-workspace-commits
- Fix-graceful-stop-task-status
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Add-web-monitoring
- Fix-stopped-task-complete-queued
- Add-responsive-web-dashboard
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Preserve-workspace-on-error
- Fix-tui-archive-loop
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Fix-tui-archive-loop
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Fix-dynamic-queue-removal
- Add-periodic-workspace-commits
- Fix-stopped-task-complete-queued
- Fix-graceful-stop-task-status
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Fix-dynamic-queue-removal
- Add-periodic-workspace-commits
- Fix-stopped-task-complete-queued
- Fix-graceful-stop-task-status
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Add-responsive-web-dashboard
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Use-shlex-for-shell-escaping
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Use-shlex-for-shell-escaping
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Update-web-port-default
- Add-tui-qr-popup
-
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Add-tui-qr-popup
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Create-execution-module
- Update-tui-archived-retention
- Refactor-archive-common
- Refactor-apply-common
- Add-parallel-hooks
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Fix-webui-state-updates
- Fix-propose-submit-crash
- Update-proposal-edit-status
- Refactor-serial-parallel-orchestration
- Update-proposal-edit-status
- Fix-propose-submit-crash
- Refactor-serial-parallel-orchestration
- Unify-event-system
- Fix-propose-submit-crash
- Refactor-serial-parallel-orchestration
- Unify-event-system
- Update-proposal-edit-status
- Proposal add-reliable-child-process-cleanup
- Proposal fix-non-empty-merge-commits
- Add-reliable-child-process-cleanup
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Add-reliable-child-process-cleanup
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Fix-non-empty-merge-commits (19/24 tasks)
-
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Fix-non-empty-merge-commits
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Update-jj-merge-workflow
- Update-apply-iteration-snapshots
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Update-parallel-processing-start-event
- Resolve config
-
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Parallel execution snapshot (auto-created by openspec-orchestrator)
- Add-tui-render-tests (apply#2)
- Add-tui-render-tests
- Fix-tui-stop-reset
- Reduce-repetitive-debug-logs (0/9 tasks, apply#1)
- Cleanup-merged-worktrees (apply#1)
- Cleanup-merged-worktrees
- Fix-tui-log-control-codes (apply#2)
- Update-parallel-commit-eligibility (apply#1)
- Apply update-apply-prompt-actionability
- Approved
- Add-command-logging (apply#1)
- Add-command-logging
- **parallel**: Define resolve goals and cleanup
- Add-tui-worktree-management (apply#1)
- Add-tui-worktree-management
- Add-serial-wip-commits (apply#1)
- Add-serial-wip-commits
- Fix-tui-archived-uncommitted-badge (apply#1)
- Fix-tui-archived-uncommitted-badge
- Fix-web-monitoring-auto-refresh (apply#1)
- Fix-web-monitoring-auto-refresh
- Fix-tui-lock-queue-while-running (apply#2)
- Fix-tui-lock-queue-while-running
- Reduce-repetitive-debug-logs
- Fix-esc-force-stop-cleanup (apply#1)
- Fix-esc-force-stop-cleanup
- Fix-archive-command-false-success
- **parallel**: Propose base-to-worktree presync before merges
- Pre-sync base into add-tui-worktree-management
- Fix-serial-archive-commit (apply#1)
- Fix-serial-archive-commit
- Pre-sync base into fix-serial-archive-commit
- Update-parallel-merge-presync
- Defer-parallel-merge-when-base-dirty
- Add-progress-stall-detector
- Add-spec-test-annotation-check
- Pre-sync base into add-progress-stall-detector
- Pre-sync base into defer-parallel-merge-when-base-dirty
- Replace-tui-plus-propose-with-worktree-command
- Fix-tui-auto-refresh-pruning (apply#1)
- **archive**: Fix-tui-auto-refresh-pruning (attempt#1)
- Fix-tui-auto-refresh-pruning
- Add-tui-resolving-status (apply#1)
- Add-tui-resolving-status
- Pre-sync base into add-tui-resolving-status
- Pre-sync base into fix-tui-auto-refresh-pruning
- Cleanup change
- Pre-sync base into replace-tui-plus-propose-with-worktree-command
- Clean up
- Remove files
- Enable-git-parallel-default (apply#1)
- Enable-git-parallel-default
- Add-worktree-branch-creation (apply#1)
- Add-worktree-branch-creation
- Fix-web-monitoring-parallel-status-updates (apply#1)
- Fix-web-monitoring-parallel-status-updates
- Add-loop-history-context (apply#1)
- Add-loop-history-context
- Add loop history context for archive and resolve operations
- Pre-sync base into add-loop-history-context
- Fix-parallel-merge-completed-status (0/0 tasks, apply#7)
- Fix-parallel-merge-completed-status
- Fix-parallel-resolve-status-display (apply#1)
- Fix-parallel-resolve-status-display
- Pre-sync base into fix-parallel-resolve-status-display
- Pre-sync base into fix-parallel-merge-completed-status
- Pre-sync base into fix-web-monitoring-parallel-status-updates
- Remove already-merged change fix-parallel-merge-completed-status
- Pre-sync base into add-worktree-branch-creation
- Pre-sync base into enable-git-parallel-default
- Fix-web-monitoring-parallel-status-updates (7/8 tasks, apply#1)
- Archive fix-web-monitoring-parallel-status-updates

- Moved future work task to separate section
- All implementation and test tasks completed
- Spec updated with changes
- Add-worktree-branch-creation (9/11 tasks, apply#2)
- Add-worktree-branch-creation (9/11 tasks, apply#3)
- Add-worktree-branch-creation (apply#4)
- Add-worktree-branch-creation
- Pre-sync base into add-worktree-branch-creation
- Fix-tui-web-state-event-forwarding (apply#1)
- Fix-tui-web-state-event-forwarding
- Improve-workspace-resume-idempotency (apply#1)
- Improve-workspace-resume-idempotency
- Pre-sync base into improve-workspace-resume-idempotency
- Pre-sync base into fix-tui-web-state-event-forwarding
- Fix-tui-web-state-event-forwarding (apply#1)
- Fix-tui-web-state-event-forwarding
- Fix-tui-web-state-event-forwarding
- Pre-sync base into fix-tui-web-state-event-forwarding
- Add-command-execution-queue (apply#3)
- Add-command-execution-queue
- Pre-sync base into add-command-execution-queue
- Add-same-error-circuit-breaker (apply#1)
- Add-same-error-circuit-breaker
- Add-same-error-circuit-breaker
- Pre-sync base into add-same-error-circuit-breaker
- Add-queue-status-merged (apply#1)
- Add-queue-status-merged
- Pre-sync base into add-queue-status-merged
- Update-queue-debounce-scheduling (apply#1)
- Update-queue-debounce-scheduling
- Pre-sync base into update-queue-debounce-scheduling
- Improve-parallel-progress-responsiveness
- Pre-sync base into improve-parallel-progress-responsiveness
- Open-proposal-file-directly (apply#1)
- Open-proposal-file-directly
- Pre-sync base into open-proposal-file-directly
- Remove-hardcoded-main-branch (apply#2)
- Remove-hardcoded-main-branch
- Pre-sync base into remove-hardcoded-main-branch
- Add-worktree-view-with-merge (apply#2)
- Add-worktree-view-with-merge
- Pre-sync base into add-worktree-view-with-merge
- Update project name, CLI command, and configuration in Conflux refactoring.

- Rename product, CLI command, Rust package, and configuration file for `OpenSpec Orchestrator` to `Conflux`
- Update Rust implementation tasks, references, configuration, messages, documentation, tests, and sample files for Conflux
- Add requirements, scenarios, behaviors, and options for TUI mode, CLI arguments, initialization, version info, and change approval management in Conflux	cli
- Specify file modifications, configuration loading order, scenario prioritization, missing settings behavior, exclusion of old file names, and worktree command setting ability for Conflux configuration
- Add-operation-iteration-to-logs (apply#1)
- Add-operation-iteration-to-logs
- Pre-sync base into add-operation-iteration-to-logs
- Add-operation-iteration-to-logs (apply#1)
- Add-operation-iteration-to-logs (cleanup)
- Add-operation-iteration-to-logs
- Rename-to-conflux (apply#1)
- Rename-to-conflux
- Pre-sync base into rename-to-conflux
- Pre-sync base into add-operation-iteration-to-logs
- Improve iteration handling and progress tracking

- Improve executor to handle apply commands in workspaces until tasks are completed
- Add stall detection for tracking progress and preventing infinite loops
- Implement hooks for pre and post apply operations in the workflow
- Enhance worktree management and merge conditions.

- Add new field `has_commits_ahead` to `WorktreeInfo` struct
- Update logic to show merge key only if there are commits ahead in worktree
- Add function to count commits ahead of base branch
- Refactor test functions for clarity and organization
- Sync-tui-logs-to-debug-file (0/7 tasks, apply#3)
- Sync-tui-logs-to-debug-file
- Sync-tui-logs-to-debug-file
- Sync-tui-logs-to-debug-file
- Add-global-resolve-lock (apply#1)
- Add-global-resolve-lock
- Fix-parallel-apply-worktree-execution
- Fix-streaming-retry (0/7 tasks, apply#3)
- Complete fix-streaming-retry: Enable retry for streaming execution

- Implement retry logic in streaming execution path
- Forward retry notifications to output channel
- Update all streaming paths (apply/archive/resolve)
- All tasks completed and tested
- Archive fix-streaming-retry

Archived completed change to openspec/changes/archive/2026-01-16-fix-streaming-retry
Updated command-queue spec with streaming retry requirements
- Fix-streaming-retry
- Pre-sync base into fix-streaming-retry
- Fix-workspace-cleanup-guard (apply#3)
- Fix-workspace-cleanup-guard
- Pre-sync base into fix-workspace-cleanup-guard
- Pre-sync base into fix-workspace-cleanup-guard
- Improve TUI Worktree View merge functionality

- Implement strict conditions for displaying the M key in TUI Worktree View
- Fix various UX issues and bugs related to merge functionality
- Improve error handling and debug logging for better stability and user experience
- Update TUI worktree merge specifications

- Implement new requirement for displaying "M: merge" key hint in worktree view
- Specify scenarios for setting warning messages for merge request failures
- Define requirement for detecting worktree commits ahead of base branch during loading
- Pre-sync base into fix-parallel-apply-worktree-execution
- Pre-sync base into add-global-resolve-lock
- Pre-sync base into sync-tui-logs-to-debug-file
- Update TUI features for resolve and merge functionality

- Added design document outlining decisions and risks for updating TUI during resolve
- Included tasks for implementing resolve logic and testing functionality
- Specified conditions for displaying key hints in different scenarios
- Fix-parallel-dynamic-queue-immediate-start (apply#1)
- Fix-parallel-dynamic-queue-immediate-start
- Pre-sync base into fix-parallel-dynamic-queue-immediate-start
- Update-tui-disable-merge-during-resolve (apply#2)
- Update-tui-disable-merge-during-resolve
- Pre-sync base into update-tui-disable-merge-during-resolve
- Update-tui-worktree-merge-status-labels (apply#1)
- Update-tui-worktree-merge-status-labels
- Update-tui-worktree-merge-status-labels
- Add-tui-stop-cancel (apply#1)
- Add-tui-stop-cancel
- Unify-ai-command-runner (apply#2)
- Unify-ai-command-runner
- Pre-sync base into unify-ai-command-runner
- Fix-web-dashboard-refresh (apply#1)
- Fix-web-dashboard-refresh
- Pre-sync base into fix-web-dashboard-refresh
- Pre-sync base into add-tui-stop-cancel
- Pre-sync base into update-tui-worktree-merge-status-labels
- Update-apply-prompt-iteration-policy (apply#1)
- Update-apply-prompt-iteration-policy
- Pre-sync base into update-apply-prompt-iteration-policy
- Update web dashboard state refresh functionality

- Introduce REST API endpoint for full orchestrator state
- Prevent stale responses by disabling HTTP caching
- Implement manual refresh functionality for TUI updates
- Sync Web API state with task progress and approval status.
- Update-worktree-default-dir (apply#1)
- Update-worktree-default-dir
- Update-worktree-default-dir
- Update-tui-log-context-headers (apply#1)
- Update-tui-log-context-headers
- Update-tui-log-context-headers
- Update-web-dashboard-state-refresh (apply#1)
- Update-web-dashboard-state-refresh
- Pre-sync base into update-web-dashboard-state-refresh
- Pre-sync base into update-tui-log-context-headers
- Pre-sync base into update-worktree-default-dir
- Refactor-execution-loop (apply#1)
- Refactor-execution-loop
- Update-webui-title-conflux (apply#1)
- Update-webui-title-conflux
- Pre-sync base into update-webui-title-conflux
- Update OpenSpec Orchestrator Web UI branding and styling

- Update application name in `app.js` to "OpenSpec Orchestrator Web Monitor"
- Update webpage title, heading, and taskbar name in `index.html` to "OpenSpec Orchestrator"
- Update meta theme color to "#1a1a2e" in `index.html`
- Update project name in comments, color variables, and spacing scale in `style.css` for consistency
- Integrate parallel apply/archive loop with common loop architecture.

- Revised and updated design, proposal, and tasks documents for integrating parallel apply/archive loop
- Defined common loop structure in `orchestration` and methods for streaming changes in parallel
- Implemented bridge between ParallelEvent and OutputHandler for logging and progress monitoring
- Update parallel command queue functionality

- Update parallel command queue behavior to include stagger in parallel apply mode
- Introduce shared CommandQueue for parallel executor to handle apply/archive execution
- Specify risks, trade-offs, and migration plan for implementing shared stagger state
- Provide detailed updates for the parallel command queue functionality in tasks.md
- Update parallel hook events implementation and design

- Create implementations and tests for parallel hook events in `tasks.md`
- Define goals and risks related to hook execution and ParallelEvent issuance in `design.md`
- Specify requirements and scenarios for parallel execution with hooks in `specs/parallel-execution/spec.md`
- Update design for unifying history and stop handling.

- Updated specifications for applying context history with parallel and serial iterations
- Added tasks for implementing common loops for history injection, stop detection, and snapshot processing
- Proposed unification of history injection and stop handling in a common loop
- Created a design plan for updating parallel history cancel WIP, including goals, decisions, risks, and migration steps
- Fix-parallel-command-queue (8/8 tasks, apply#3)
- Update-apply-cwd-resolution (apply#2)
- Update-apply-cwd-resolution
- Update-apply-cwd-resolution
- Update-worktree-default-base (apply#1)
- Update-worktree-default-base
- Update-apply-task-progress (apply#1)
- Update-apply-task-progress
- Update-parallel-hook-events (apply#1)
- Update-parallel-hook-events
- Update-parallel-hook-events
- Update-webui-title-conflux (apply#1)
- Update-webui-title-conflux
- Update-parallel-command-queue (apply#1)
- Update-parallel-command-queue
- Update-parallel-apply-archive-loop (apply#1)
- Update-parallel-apply-archive-loop
- Update-parallel-apply-archive-loop
- Update-parallel-history-cancel-wip (4/4 tasks, apply#1)
- Update-parallel-history-cancel-wip
- Fix-merged-state-detection (apply#1)
- Fix-merged-state-detection
- Fix-parallel-command-queue
- Update-change-list-proposal-filter (apply#1)
- Update-change-list-proposal-filter
- Update-change-list-proposal-filter
- Fix-mergewait-resolve-running (apply#1)
- Fix-mergewait-resolve-running
- Fix-mergewait-resolve-running
- Pre-sync base into fix-mergewait-resolve-running
- Pre-sync base into update-change-list-proposal-filter
- Pre-sync base into fix-parallel-command-queue
- Pre-sync base into fix-merged-state-detection
- Pre-sync base into update-parallel-history-cancel-wip
- Pre-sync base into update-parallel-apply-archive-loop
- Pre-sync base into update-parallel-command-queue
- Pre-sync base into update-webui-title-conflux
- Pre-sync base into update-parallel-hook-events
- Pre-sync base into update-apply-task-progress
- Pre-sync base into update-worktree-default-base
- Pre-sync base into update-apply-cwd-resolution
- Fix-merged-state-detection
- Fix-merged-state-detection
- Pre-sync base into fix-merged-state-detection
- Update-resolve-prompt-no-verify (apply#1)
- Update-resolve-prompt-no-verify
- Update-resolve-prompt-no-verify
- Update-tui-task-progress-retention (apply#1)
- Update-tui-task-progress-retention
- Update-tui-task-progress-retention
- Pre-sync base into update-tui-task-progress-retention
- Pre-sync base into update-resolve-prompt-no-verify
- Add-worktree-setup-script (apply#1)
- Add-worktree-setup-script
- Add-worktree-setup-script
- Update-apply-prompt-no-verify (apply#1)
- Update-apply-prompt-no-verify
- Update-apply-prompt-no-verify
- Pre-sync base into add-worktree-setup-script
- Update-archive-merge-guards (apply#1)
- Update-archive-merge-guards
- Update-archive-merge-guards
- Pre-sync base into update-archive-merge-guards
- Update-archive-merge-guards
- Update-archive-merge-guards
- Update-worktree-setup-on-tui-create (apply#1)
- Update-worktree-setup-on-tui-create
- Update-worktree-setup-on-tui-create
- Update-worktree-setup-on-tui-create
- Update-worktree-setup-on-tui-create
- Improve worktree setup process in TUI workflow

- Add new spec, tasks, and proposal for updating worktree setup on TUI create
- Define system requirements, scenarios, and tasks for TUI create workflow setup
- Include handling for setup failure and existing worktree creation errors
- Describe the proposed changes and their impact in detail
- Fix-archive-verification-changes-remain (apply#1)
- Fix-archive-verification-changes-remain
- Fix-archive-verification-changes-remain
- Pre-sync base into fix-archive-verification-changes-remain
- Pre-sync base into update-worktree-setup-on-tui-create
- Improve TUI resolve wait status display

- Create new specifications file `spec.md` for Event-Driven State Updates in TUI architecture
- Define visual identification changes for waiting state in TUI
- Clarify display of resolve waiting status in TUI for improved user understanding
- Clean up whitespace in tasks.md
- Fix-parallel-concurrency-limit
- Fix-parallel-concurrency-limit
- Pre-sync base into fix-parallel-concurrency-limit
- Update-tui-resolve-wait-status (apply#3)
- Update-tui-resolve-wait-status
- Pre-sync base into update-tui-resolve-wait-status
- Enhance worktree progress and apply WIP snapshots

- Create design and proposal documents for fixing worktree archive progress
- Define goals and non-goals for updating apply WIP with `--no-verify`
- Specify requirements for `--no-verify` in apply prompts and WIP snapshots
- Update-apply-wip-no-verify
- Pre-sync base into update-apply-wip-no-verify
- Update-log-headers-iteration (apply#1)
- Update-log-headers-iteration
- Update-log-headers-iteration
- Pre-sync base into update-log-headers-iteration
- Update handling of uncommitted warnings in logs for TUI

- Add proposal for handling uncommitted changes warning through logs only in TUI
- Specify changes to show uncommitted warnings only in TUI logs, leaving CLI warnings unchanged
- Implement logging uncommitted warnings in `src/tui/state/events.rs` without popup display
- Update warning popup related tests in `src/tui/state/mod.rs`
- Avoid-merge-abort-conflict-check
- Update-uncommitted-warning-logs-only (apply#1)
- Update-uncommitted-warning-logs-only
- Remove deprecated CLI flags and enhance help output

- Remove --opencode-path and --openspec-cmd CLI flags
- Remove OPENSPEC_CMD environment variable support
- Enhance cflx --help with all subcommands and main options
- Document --web/--web-port/--web-bind flags in help
- Clean up unused test imports and functions
- Update CLI and configuration specs accordingly

All tests pass and OpenSpec validation succeeds.
- Translate change specs to English and change to ADDED requirements
- Remove-cli-openspec-flags
- Remove-cli-openspec-flags
- Pre-sync base into remove-cli-openspec-flags
- Pre-sync base into update-uncommitted-warning-logs-only
- Pre-sync base into avoid-merge-abort-conflict-check
- Fix-worktree-archive-progress (apply#1)
- Fix-worktree-archive-progress
- **archive**: Fix-worktree-archive-progress (attempt#1)
- **archive**: Fix-worktree-archive-progress (attempt#2)
- **archive**: Fix-worktree-archive-progress (attempt#3)
- Complete archive: remove original change directory
- Remove-archived-change-dir (apply#1)
- Remove-archived-change-dir
- **archive**: Remove-archived-change-dir (attempt#1)
- **archive**: Remove-archived-change-dir (attempt#2)
- Remove-archived-change-dir
- Pre-sync base into remove-archived-change-dir
- Update-tui-worktree-enter-logging (apply#1)
- Update-tui-worktree-enter-logging
- Update-tui-worktree-enter-logging
- Pre-sync base into update-tui-worktree-enter-logging
- Update worktree conflict check and history logs specifications

- Update worktree merge eligibility tasks and specifications
- Update worktree conflict check tasks and specifications
- Add new requirements for Log Entry Structure and Display in TUI architecture
- Modify system prompts for apply and archive commands in loop history logs update
- Update TUI worktree merge eligibility specifications with error message requirements
- Update archive state handling and execution behaviors.

- Add requirements for Future Work restrictions and task movement clarifications
- Update archiving logic for better clarity and efficiency
- Implement change skipping mechanism for failed changes and dependencies
- Include event logging and TUI display for skipped changes
- Improve conflict detection in worktree checks

- Deleted unnecessary tasks.md file for update worktree conflict check
- Renamed proposal.md to 2026-01-18-update-worktree-conflict-check/proposal.md
- Added tasks for implementing worktree conflict check updates
- Updated `check_merge_conflicts` function to use `git merge-tree` without modifying working tree
- Improved error handling and logging for conflict detection and successful merge info
- Update-parallel-apply-worktree-guard
- Pre-sync base into update-parallel-apply-worktree-guard
- Update-loop-history-logs (apply#1)
- Update-loop-history-logs
- Update-error-context-messages (apply#2)
- Update-error-context-messages
- Update-tui-worktree-enter-logging (apply#1)
- Update-tui-worktree-enter-logging
- Update-tui-worktree-enter-logging
- Update-loop-history-logs
- Pre-sync base into update-loop-history-logs
- Pre-sync base into update-tui-worktree-enter-logging
- Pre-sync base into update-error-context-messages
- Update-tui-merged-progress-refresh (apply#1)
- Update-tui-merged-progress-refresh
- Update-tui-merged-progress-refresh
- Pre-sync base into update-tui-merged-progress-refresh
- Update-parallel-reanalysis-order
- Update-parallel-reanalysis-order
- Update CLI spec with history output tail summary

- Implemented new requirements for updating command output tail in history prompt
- Added tasks for implementation and validation steps
- Provided proposal for injecting command output tail into history prompt and justifying the need for stdout/stderr summary for retry scenarios
- Pre-sync base into update-parallel-reanalysis-order
- Add-utc-build-number (apply#1)
- Add-utc-build-number
- Fix-merge-resolve-status (apply#1)
- Fix-merge-resolve-status
- Fix-merge-resolve-status
- Update TUI processing progress specifications

- Add tasks.md and proposal.md files for updating TUI processing progress
- Define implementation tasks, validation steps, and scenarios for TUI architecture updates
- Modify requirements for event-driven state updates in TUI architecture
- Pre-sync base into fix-merge-resolve-status
- Pre-sync base into add-utc-build-number
- Audit-parallel-reanalysis-gaps
- Refactor-web-monitoring-parity (apply#2)
- Refactor-web-monitoring-parity
- Refactor-web-monitoring-parity
- Update-tui-processing-progress (apply#1)
- Update-tui-processing-progress
- Update-history-output-tail (apply#1)
- Update-history-output-tail
- Update-history-output-tail
- Pre-sync base into update-history-output-tail
- Pre-sync base into update-tui-processing-progress
- Pre-sync base into refactor-web-monitoring-parity
- Pre-sync base into audit-parallel-reanalysis-gaps
- Add-acceptance-loop
- Add-acceptance-loop
- Add-utc-build-number (apply#1)
- Add-utc-build-number (manual cleanup - already merged)
- Add-utc-build-number
- Pre-sync base into add-utc-build-number
- Pre-sync base into add-acceptance-loop
- Integrate acceptance testing into orchestration flow

- Add acceptance integration specifications for CLI success, failure, and command execution
- Integrate acceptance testing into orchestration flow with design document and proposal
- Define tasks for sequential and parallel execution incorporating acceptance testing
- Implement branching logic, clear logging, and state transitions for acceptance failures/cmd failures
- Add-acceptance-integration
- Update progress tracking and handling in specs and proposals

- Updated specifications for broadcasting state updates via WebSocket
- Modified TUI architecture to handle progress updates and failures
- Added proposal and tasks for progress updates and testing
- Pre-sync base into add-acceptance-integration
- Update-progress-archive-resolve (apply#1)
- Update-progress-archive-resolve
- Pre-sync base into update-progress-archive-resolve
- Fix-parallel-queue-reanalysis (apply#1)
- Fix-parallel-queue-reanalysis
- Fix-parallel-queue-reanalysis
- Update acceptance failure behavior and task handling

- Implement new file `tasks.md` for acceptance-fail-apply-loop directory
- Define rules for task updates and failure reasons
- Add specification for parallel execution acceptance loop
- Specify changes to acceptance behavior in CLI orchestration loop
- Pre-sync base into fix-parallel-queue-reanalysis
- Acceptance-fail-apply-loop
- Web-ui-status-align-and-summary
- Update-resume-acceptance (apply#1)
- Update-resume-acceptance
- Update-resume-acceptance
- Pre-sync base into update-resume-acceptance
- Fix-parallel-queue-reanalysis (apply#1)
- Fix-parallel-queue-reanalysis
- Add-acceptance-continue-state (apply#1)
- Add-acceptance-continue-state
- Add-acceptance-continue-state
- Enhance TUI progress fallback functionality

- Created new spec.md file outlining requirements and scenarios for event-driven state updates in TUI architecture
- Added tasks.md file for TUI worktree fallback wiring with unified progress retention logic
- Included proposal.md document detailing the rationale and impact of enhancing TUI progress fallback functionality
- Pre-sync base into add-acceptance-continue-state
- Pre-sync base into fix-parallel-queue-reanalysis
- Pre-sync base into web-ui-status-align-and-summary
- Pre-sync base into acceptance-fail-apply-loop
- Add-tui-accepting-status (apply#1)
- Add-tui-accepting-status
- Pre-sync base into add-tui-accepting-status
- Update-acceptance-default-continue
- Update-acceptance-default-continue
- Fix-order-based-slot-launch (apply#3)
- Fix-order-based-slot-launch
- Pre-sync base into fix-order-based-slot-launch
- Add-change-delta-conflict-check
- Update-resume-acceptance (apply#1)
- Update-resume-acceptance
- Update-resume-acceptance
- Fix-tui-progress-fallback (apply#1)
- Fix-tui-progress-fallback
- Fix-tui-progress-fallback
- Pre-sync base into fix-tui-progress-fallback
- Pre-sync base into update-resume-acceptance
- Pre-sync base into add-change-delta-conflict-check
- Fix-merged-analysis-loop (apply#1)
- Fix-merged-analysis-loop
- Pre-sync base into fix-merged-analysis-loop
- Stall-merge-timeout-circuit-breaker (apply#1)
- Stall-merge-timeout-circuit-breaker
- Add-log-headers-analysis-resolve (apply#1)
- Add-log-headers-analysis-resolve
- Pre-sync base into add-log-headers-analysis-resolve
- Pre-sync base into stall-merge-timeout-circuit-breaker
- Fix-slot-availability-count (apply#1)
- Fix-slot-availability-count
- Add-acceptance-iteration-header (apply#1)
- Add-acceptance-iteration-header
- Update task progress handling in multiple scenarios.

- Updated requirements and scenarios for maintaining task progress in various scenarios
- Added proposal document outlining changes to address tasks progress issue
- Defined behavior for reflecting progress in the TUI during Archive/Resolve phase
- Pre-sync base into add-acceptance-iteration-header
- Pre-sync base into fix-slot-availability-count
- Update-tasks-progress-archive-resolve (apply#1)
- Update-tasks-progress-archive-resolve
- Update-tasks-progress-archive-resolve
- Add-web-api-openapi-docs (apply#1)
- Add-web-api-openapi-docs
- Add-web-api-openapi-docs
- - Refactor reanalysis trigger system for improved responsiveness and control
- Define new requirements for parallel analysis targeting
- Implement redesign proposal with changes to trigger conditions and scheduling mechanism
- Update orchestration behavior for `MergeWait` in specs

- Added new proposal file for `merge_wait` behavior in orchestration
- Clarified impact on affected specs and code
- Listed implementation tasks for updating merge-wait handling and orchestration behavior
- Pre-sync base into add-web-api-openapi-docs
- Pre-sync base into update-tasks-progress-archive-resolve
- Refactor-reanalysis-trigger (apply#1)
- Refactor-reanalysis-trigger
- Refactor-reanalysis-trigger
- Pre-sync base into refactor-reanalysis-trigger
- Update-tui-exit-shortcut (apply#1)
- Update-tui-exit-shortcut
- Add-merge-wait-auto-clear (apply#2)
- Add-merge-wait-auto-clear
- Improve re-analysis trigger logic and parallel execution.

- Added new files `tasks.md`, `design.md`, `spec.md`, and `proposal.md` related to reanalysis trigger fixes
- Implemented asynchronous trigger notification and separated dependencies for re-analysis
- Refactored debounce logic and prevented frequent re-analysis during consecutive notifications
- Pre-sync base into add-merge-wait-auto-clear
- Pre-sync base into update-tui-exit-shortcut
- Reanalysis-trigger-fix (apply#1)
- Reanalysis-trigger-fix
- Pre-sync base into reanalysis-trigger-fix
- Revamp TUI stopped queue policy specifications.

- Added tasks for updating queue status and execution marks during stopped transition
- Included proposal for changing TUI stopped queue policy with clarified transition states and behaviors
- Defined scenarios for Stopped mode display, queue management, and task completion behavior in new CLI specification file
- Merge-wait-does-not-stop-orchestration (apply#1)
- **archive**: Merge-wait-does-not-stop-orchestration (attempt#1)
- **archive**: Merge-wait-does-not-stop-orchestration (attempt#2)
- Merge-wait-does-not-stop-orchestration
- Improve TUI Stopped Resume Policy

- Introduce new spec for `TUI Stopped Resume` with defined requirements
- Update proposal with clearer behavior for transitioning from STOPPED state
- Add tasks for CLI spec diff creation and unit test updates
- Update-tui-stopped-resume (apply#1)
- Update-tui-stopped-resume
- Update-tui-stopped-resume
- Update-log-iteration-headers (apply#1)
- Update-log-iteration-headers
- Tui-stopped-queue-policy (apply#2)
- Tui-stopped-queue-policy
- Update log iteration headers across specifications

- Add new specification file for TUI architecture
- Update log entry structure requirements
- Propose changes to include iteration numbers in log outputs
- Define impact on affected specs and code files
- Pre-sync base into tui-stopped-queue-policy
- Pre-sync base into update-log-iteration-headers
- Pre-sync base into update-tui-stopped-resume
- Fix-parallel-reanalysis-dispatch (apply#1)
- Fix-parallel-reanalysis-dispatch
- Fix-parallel-reanalysis-dispatch
- Pre-sync base into fix-parallel-reanalysis-dispatch
- Fix-archive-tasks-fallback (apply#1)
- Fix-archive-tasks-fallback
- Update-web-ui-change-status (apply#2)
- Update-web-ui-change-status
- Update-web-ui-change-status
- Pre-sync base into update-web-ui-change-status
- Pre-sync base into fix-archive-tasks-fallback
- Remove-parallel-batch-processing
- Update TUI status display for parallel execution.

- Add updated requirements and scenarios for TUI status display in parallel execution and CLI specs
- Introduce tasks for implementing TUI status display updates
- Outline proposal for updating TUI orchestration status display reasons and consequences
- Fix-parallel-cancel-worktree-cleanup (apply#1)
- Fix-parallel-cancel-worktree-cleanup
- Pre-sync base into fix-parallel-cancel-worktree-cleanup
- Update-log-iteration-headers (apply#3)
- Update-log-iteration-headers
- Update-log-iteration-headers
- Update-tui-status-display (apply#2)
- Update-tui-status-display
- Fix-parallel-cancel-worktree-cleanup (apply#1)
- Fix-parallel-cancel-worktree-cleanup
- Fix-parallel-cancel-worktree-cleanup
- Update TUI/Web UI status labels across specs

- Add detailed proposal file for updating TUI/Web UI status labels
- Define unified set of display vocabulary for status indicators
- Specify tasks for unifying vocabulary, updating status events, and implementing status:iteration format
- Update CLI and Web monitoring specifications
- Pre-sync base into fix-parallel-cancel-worktree-cleanup
- Pre-sync base into update-tui-status-display
- Pre-sync base into update-log-iteration-headers
- Fix-reanalysis-concurrent-dispatch (11/12 tasks, apply#5)
- Complete acceptance verification for fix-reanalysis-concurrent-dispatch

- All 913 tests pass successfully
- Implementation verified against specification
- No acceptance failures found
- Fix-reanalysis-concurrent-dispatch (12/12 tasks, apply#6)
- Fix-reanalysis-concurrent-dispatch
- Fix-reanalysis-concurrent-dispatch
- Improve TUI archive progress maintenance and conflict checks

- Implement updates to maintain progress display during TUI archive process
- Add auto-refresh feature for Worktree list to check for conflicts
- Prioritize stdout for conflict detection in `check_merge_conflicts` function
- Improve git merge-tree conflict check fix proposal

- Propose solutions for fixing git merge-tree conflict check failure
- Specify goals and risks of using git merge-tree with specific arguments
- Outline migration plan for replacing existing conflict check process
- Improve progress preservation during TUI archive process

- Update progress display to preserve values during automatic updates in TUI
- Modify logic in `runner.rs` to handle 0/0 progress values
- Add tasks for implementation of worktree deletion while running
- Pre-sync base into fix-reanalysis-concurrent-dispatch
- Update parallel execution acceptance failure handling

- Update scenario titles and acceptance output indicators for parallel acceptance recording failure
- Adjust behavior for recording acceptance output tail in case of failure
- Improve selection process for next change in parallel execution mode
- Enhance agent task list specifications.

- Include 'Manual TUI verification' in tasks requiring human work
- Provide contextual information on operational constraints and task movement criteria
- Fix-merge-tree-conflict-check (apply#1)
- Fix-merge-tree-conflict-check
- Fix-merge-tree-conflict-check
- Fix-tui-archive-progress-zero
- Fix-tui-archive-progress-zero
- Pre-sync base into fix-merge-tree-conflict-check
- Fix-merge-wait-resolve-flow
- Fix-merge-wait-resolve-flow
- Refactor-agent-module-split (0/5 tasks, apply#1)
- Refactor-agent-module-split (apply#2)
- Refactor-agent-module-split
- Refactor-agent-module-split
- Update workspace cleanup policy and parallel execution spec.

- Update file names and move files to the archive folder to reflect 2026 archival
- Clarify workspace cleanup requirements and preservation rules in spec.md files
- Document parallel execution behavior after force stops and successful merges
- Pre-sync base into refactor-agent-module-split
- Refactor-shared-progress-helpers
- Refactor-shared-progress-helpers
- Refactor-shared-progress-helpers
- Refactor-tui-state-events-split (apply#1)
- Refactor-tui-state-events-split
- Pre-sync base into refactor-tui-state-events-split
- Pre-sync base into refactor-shared-progress-helpers
- Add-accepting-spinner (apply#1)
- Add-accepting-spinner
- Pre-sync base into add-accepting-spinner
- Enhance rendering of logs in TUI.

- Improve log title formatting in the TUI
- Add auto-scroll status indicator for improved user experience
- Refactor-vcs-git-commands-split (apply#1)
- Refactor-vcs-git-commands-split
- Refactor-vcs-git-commands-split
- Refactor-vcs-git-commands-split
- Remove-completed-status (0/9 tasks, apply#1)
- Remove-completed-status
- Pre-sync base into refactor-vcs-git-commands-split
- Allow-worktree-delete-while-running (apply#1)
- Allow-worktree-delete-while-running
- Fix-tui-accepting-stop-status (2/3 tasks, apply#1)
- Fix-tui-accepting-stop-status
- Fix-tui-accepting-stop-status
- Fix-tui-accepting-stop-status
- Pre-sync base into fix-tui-accepting-stop-status
- Pre-sync base into allow-worktree-delete-while-running
- Implement-web-tui-status-labels
- Update acceptance follow-up formatting specifications

- Update acceptance follow-up formatting tasks in `tasks.md`
- Define detailed CLI spec requirements for orchestration loop execution in `spec.md`
- Propose changes to acceptance failure follow-up formatting in `proposal.md`
- Refactor-parallel-module-split (apply#1)
- Refactor-parallel-module-split
- Update re-analysis slot gating specifications and tasks.

- Implemented proposed changes for re-analysis slot gating
- Added new specs for parallel execution requirements
- Defined implementation and validation tasks for the update in tasks.md
- Pre-sync base into refactor-parallel-module-split
- Update-acceptance-followup-formatting (apply#1)
- Update-acceptance-followup-formatting
- Pre-sync base into update-acceptance-followup-formatting
- Pre-sync base into implement-web-tui-status-labels
- Refactor-orchestrator-state (apply#1)
- Refactor-orchestrator-state
- Update-worktree-delete-branch (apply#1)
- Update-worktree-delete-branch
- Pre-sync base into update-worktree-delete-branch
- Update-reanalysis-slot-gating (apply#1)
- Update-reanalysis-slot-gating
- Pre-sync base into update-reanalysis-slot-gating
- Update default acceptance max continues value in config.

- Increase `DEFAULT_ACCEPTANCE_MAX_CONTINUES` from `2` to `10` across different modules
- Implement event forwarding for UI changes in orchestrator.rs
- Update queue configuration tests in mod.rs
- Update acceptance stagger execution details

- Add new files `specs/command-queue/spec.md`, `tasks.md`, and `proposal.md`
- Define requirements, scenarios, and tasks related to updating acceptance stagger start
- Specify changes for parallel execution modes and stagger sharing in acceptance execution
- Pre-sync base into refactor-orchestrator-state
- Refactor-serial-run-service (apply#1)
- Refactor-serial-run-service
- Pre-sync base into refactor-serial-run-service
- Update-acceptance-diff-check
- Add-web-ui-execution-controls
- Pre-sync base into add-web-ui-execution-controls
- Pre-sync base into update-acceptance-diff-check
- Update-dependency-blocked-status (apply#1)
- Update-dependency-blocked-status
- Update-acceptance-stagger-start (apply#1)
- Update-acceptance-stagger-start
- Pre-sync base into update-acceptance-stagger-start
- Pre-sync base into update-dependency-blocked-status
- Update-config-xdg-priority
- Update acceptance failure follow-up format across files

- Add new requirements for Orchestration loop behavior
- Define tasks for handling acceptance failure follow-up
- Include implementation tasks for updating follow-up format
- Specify changes to unify format for better tracking of acceptance failure tasks
- Update-tui-apply-iteration-sync (apply#1)
- Update-tui-apply-iteration-sync
- Add-on-merged-hook
- Update TUI apply iteration sync specifications

- Add new specification file for updating TUI apply iteration sync
- Define requirements for Terminal Status Task Count Display in TUI
- Update TUI to handle iteration values in various modules
- Pre-sync base into add-on-merged-hook
- Update-task-parser-excluded-sections
- Update-parallel-event-forwarding
- Update-acceptance-followup-format
- Pre-sync base into update-acceptance-followup-format
- Pre-sync base into update-parallel-event-forwarding
- Pre-sync base into update-task-parser-excluded-sections
- Pre-sync base into update-tui-apply-iteration-sync
- Update-acceptance-followup-authoring (apply#1)
- Update-acceptance-followup-authoring
- Update-tui-apply-iteration-sync
- Pre-sync base into update-tui-apply-iteration-sync
- Add-acceptance-git-clean-check (apply#1)
- Add-acceptance-git-clean-check
- Update-running-count-and-resolve-slots (apply#1)
- Update-running-count-and-resolve-slots
- Pre-sync base into update-running-count-and-resolve-slots
- Pre-sync base into add-acceptance-git-clean-check
- Pre-sync base into update-acceptance-followup-authoring
- Update-log-autoscroll (apply#1)
- Update-log-autoscroll
- Update-acceptance-followup-authoring (apply#1)
- Update-acceptance-followup-authoring
- 0.4.1
- 0.4.2
- Add-acceptance-git-clean-check (apply#1)
- Add-acceptance-git-clean-check
- Pre-sync base into add-acceptance-git-clean-check
- Pre-sync base into add-acceptance-git-clean-check
- Pre-sync base into add-acceptance-git-clean-check
- Pre-sync base into update-acceptance-followup-authoring
- Pre-sync base into update-log-autoscroll
- Add-acceptance-tail-to-apply (apply#2)
- Add-acceptance-tail-to-apply (apply#3)
- Add-acceptance-tail-to-apply
- Update-workspace-archive-detection (apply#3)
- Update-workspace-archive-detection
- Update workspace archive detection logic across files.

- Update workspace archive detection logic to use file status instead of commit messages
- Refactor and update test cases for detection logic
- Specify actions for archiving and archived scenarios in parallel execution area
- Pre-sync base into update-workspace-archive-detection
- Add-resolve-wait-status (apply#1)
- Add-resolve-wait-status
- Fix-parallel-on-merged-hook (apply#3)
- Fix-parallel-on-merged-hook (apply#2)
- Fix-parallel-on-merged-hook
- Pre-sync base into fix-parallel-on-merged-hook
- Pre-sync base into add-resolve-wait-status
- Pre-sync base into add-acceptance-tail-to-apply
- Add-acceptance-base-diff (apply#2)
- Add-acceptance-base-diff
- Add-tui-command-logs (apply#3)
- Add-tui-command-logs
- Pre-sync base into add-tui-command-logs
- Pre-sync base into add-acceptance-base-diff
- Test-on-merged-hook
- Test-on-merged-hook-parallel
- Test-hook-fire
- Test-hook-simple
- Test-hook-simple
- Update-acceptance-external-dependency-policy (0/8 tasks, apply#1)
- Update-acceptance-external-dependency-policy (apply#2)
- Update-acceptance-external-dependency-policy
- Pre-sync base into update-acceptance-external-dependency-policy

### Refactoring

- Simplify orchestrator architecture and add openspec integration
- **tui**: Replace ToggleApproval with mode-specific approval commands
- **executor**: Replace blocking command execution with async streaming
- **tui**: Simplify change state refresh logic
- Refactor code for improved readability
- Execute AI agent commands in workspace directory instead of repo root
- Refactor project directory structure and file names

- Renamed files and directories related to `update-parallel-history-cancel-wip` to use the date `2026-01-17` for archiving purposes.
- Updated messages and scenarios in `specs/cli/spec.md` related to applying context history, creating a WIP, and archiving context history in parallel mode.
- Refactor and enhance parallel execution functionality
- Refactor completion state transitions and naming conventions

- Update completion state transition conditions and TUI behavior
- Keep track of last known task counts for changes transitioning to Completed, Archived, or Merged state
- Renamed files for better organization and archival purposes
- Refactor workspace orchestration and UI rendering in `Conflux` app

- Add new default value for `OrchestratorConfig`
- Adjust `OrchestratorConfig` to include `workspace_base_dir`
- Improve error handling for forbidden archive directories
- Update header name in UI to "Conflux"
- Implement tests for displaying worktree delete confirm modal
- Enhance thread safety in environment locks
- Implement parallel concurrency limit for workspace creation
- Update archive verification logic and test coverage
- Refactor codebase to remove archived change directories

- Add tasks, proposal, and specifications for removing archived change directories
- Define the reasons, impacted specs, and affected code files
- Specify requirements for completely deleting change directories after archiving
- Refactor TUI worktree merge eligibility specifications
- Refactor web monitoring parity across files

- Refactored web monitoring parity to align Web UI with TUI
- Added tasks and proposal documents for implementing real-time monitoring features
- Included specifications for WebSocket integration and broadcasting real-time updates between TUI and Web UI
- Optimize parallel processing module
- Update acceptance pass detection specifications
- Remove parallel batch processing assumptions
- Refactor module responsibilities for better maintainability
- Refactor workspace cleanup behavior and legacy prompt
- Refactor prompt building process

- Refactor `runner.rs` to include `full_prompt` building with user_prompt, system_prompt, and history_context
- Update `prompt.rs` to add a parameter `change_id` and replace `{change_id}` in `ACCEPTANCE_SYSTEM_PROMPT`
- Update `defaults.rs` to add `{change_id}` placeholder and update prompts in `ACCEPTANCE_SYSTEM_PROMPT`
- Refactor orchestrator state for shared model integration.
- Refactor tui state and runner codebase

- Add new method `toggle_approval` for changing approval status
- Implement methods for worktree cursor movement
- Modify the `run_tui_loop` function with various changes related to orchestrator events, worktrees, commands, and logs
- Update config default checks and guidelines
- Update default configurations and regression tests
- Refactor SerialRunService pattern into new service
- Refactor error handling and method calls in executor and output modules
- Refactor acceptance diff check process

- Update acceptance diff check design document and specifications.
- Introduce new files for parallel execution and command line interface specifications.
- Implement changes to acceptance flow for narrowing down review scope and saving commit identifiers.
- Refactor version string display in `utils.rs`
- Optimize code for improved performance and readability
- Refactor serial execution and update dependency statuses
- Refactor CLI struct and TUI functionality

- Added new struct fields and tests in `cli.rs`
- Implemented web monitoring server functionality in `main.rs`
- Updated configuration loading and URL handling in `main.rs`
- Update running count and resolve slots in parallel execution.
- Refactor acceptance flow and prompt handling across files

- Consolidated acceptance output logic in multiple files
- Added last output context parameter to acceptance prompt building
- Improved history tracking for acceptance attempts and output tails
- **web**: Optimize CSS transitions and improve text wrapping
- Refactor OpenSpec command files in .claude/commands/cflx

- Added new files for handling OpenSpec changes in Conflux
- Included guidelines and instructions for archiving, proposing, and applying changes
- Emphasized the importance of following the defined steps and guidelines for each process

### Styling

- Improve visibility by increasing Logs section height.

### Testing

- Revert to zlnynurz to debug
