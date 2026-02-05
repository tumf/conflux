# Changelog

All notable changes to this project will be documented in this file.


## [0.4.19] - 2026-02-05

    ### Documentation

        - Update release documentation and workflow
        - **openspec**: Add change for resolve pending MergeDeferred

    ### Features

        - Update project version and release configurations across files

    ### Miscellaneous

        - Add cargo-release config and changelog
        - Improve Makefile with cargo-release integration
        - Update project version to 0.4.15 and enhance release workflow
        - Update release configuration metadata in Cargo.toml
        - Update project version and release configuration metadata
        - Update project version and release configuration metadata


## [0.4.14] - 2026-02-05

    ### Documentation

        - **openspec**: Add update-tui-resolve-queue change set


## [0.4.13] - 2026-02-04

    ### Documentation

        - **openspec**: Add uncommitted change detection proposal


## [0.4.11] - 2026-02-04

    ### Documentation

        - Document acceptance and resolve commands
        - **openspec**: Add in-flight deps analysis proposal
        - **openspec**: Add TUI iteration guard change draft

    ### Miscellaneous

        - Add skill scaffolding and conflux skills
        - Add cflx config and workflow skills


## [0.4.9] - 2026-02-01

    ### Archive

        - Refactor-parallel-scheduler-loop
        - Refactor-orchestrator-run-loop
        - Refactor-tui-runner-handlers
        - Refactor-archive-loop-helpers
        - Refactor-tui-state-guards
        - Refactor-orchestrator-run-loop
        - Refactor-tui-state-guards
        - Refactor-orchestrator-run-loop
        - Refactor-parallel-scheduler-loop
        - Refactor-archive-loop-helpers
        - Fix-tui-logs-wrap
        - Fix-tui-logs-wrap
        - Refactor-command-queue-retry
        - Refactor-serial-run-service-flow
        - Refactor-analyzer-streaming-parse
        - Refactor-orchestrator-run-loop
        - Refactor-tui-handler-deps
        - Refactor-parallel-run-service-prep
        - Fix-tui-ready-return

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

    ### WIP

        - Update resolve merge to cleanup resurrected openspec/changes

    ### Implement

        - TUI error mode transition and MergeWait operation consistency

    ### Revert

        - **tui**: Remove log preview from change list rows

    ### Tasks

        - Update acceptance #2 failure follow-up tasks


## [0.4.3] - 2026-01-29

    ### Apply

        - Add-git-worktree-parallel
        - Sync-tui-logs-to-debug-file
        - Fix-merge-wait-resolve-flow
        - Refactor-shared-progress-helpers
        - Fix-tui-accepting-stop-status

    ### Archive

        - Refactor-config-module
        - Refactor-tui-state
        - Refactor-vcs-abstraction
        - Sync-readme-translations
        - Fix-cli-spec-language
        - Remove-dummy-spec
        - Clear-new-badge-on-interaction
        - Refactor-tui-key-hints-layout
        - Fix-stopped-mode-approval-queue
        - Add-missing-spec-tests
        - Add-propose-command
        - Add-propose-command
        - Fix-stopped-mode-approval-queue
        - Add-workspace-resume
        - Add-missing-spec-tests
        - Preserve-workspace-on-error
        - Fix-dynamic-queue-removal
        - Add-periodic-workspace-commits
        - Fix-graceful-stop-task-status
        - Add-web-monitoring
        - Fix-stopped-task-complete-queued
        - Add-responsive-web-dashboard
        - Preserve-workspace-on-error
        - Fix-tui-archive-loop
        - Fix-tui-archive-loop
        - Fix-dynamic-queue-removal
        - Add-periodic-workspace-commits
        - Fix-stopped-task-complete-queued
        - Fix-graceful-stop-task-status
        - Fix-dynamic-queue-removal
        - Add-periodic-workspace-commits
        - Fix-stopped-task-complete-queued
        - Fix-graceful-stop-task-status
        - Add-responsive-web-dashboard
        - Use-shlex-for-shell-escaping
        - Use-shlex-for-shell-escaping
        - Add-tui-qr-popup
        - Add-tui-qr-popup
        - Create-execution-module
        - Refactor-archive-common
        - Refactor-apply-common
        - Add-parallel-hooks
        - Fix-webui-state-updates
        - Fix-propose-submit-crash
        - Refactor-serial-parallel-orchestration
        - Fix-propose-submit-crash
        - Refactor-serial-parallel-orchestration
        - Unify-event-system
        - Fix-propose-submit-crash
        - Refactor-serial-parallel-orchestration
        - Unify-event-system
        - Add-reliable-child-process-cleanup
        - Add-reliable-child-process-cleanup
        - Fix-non-empty-merge-commits
        - Add-tui-render-tests
        - Fix-tui-stop-reset
        - Cleanup-merged-worktrees
        - Add-command-logging
        - Add-tui-worktree-management
        - Add-serial-wip-commits
        - Fix-tui-archived-uncommitted-badge
        - Fix-web-monitoring-auto-refresh
        - Fix-tui-lock-queue-while-running
        - Reduce-repetitive-debug-logs
        - Fix-esc-force-stop-cleanup
        - Fix-archive-command-false-success
        - Fix-serial-archive-commit
        - Defer-parallel-merge-when-base-dirty
        - Add-progress-stall-detector
        - Add-spec-test-annotation-check
        - Replace-tui-plus-propose-with-worktree-command
        - Fix-tui-auto-refresh-pruning
        - Add-tui-resolving-status
        - Enable-git-parallel-default
        - Add-worktree-branch-creation
        - Fix-web-monitoring-parallel-status-updates
        - Add-loop-history-context
        - Fix-parallel-merge-completed-status
        - Fix-parallel-resolve-status-display
        - Add-worktree-branch-creation
        - Fix-tui-web-state-event-forwarding
        - Improve-workspace-resume-idempotency
        - Fix-tui-web-state-event-forwarding
        - Fix-tui-web-state-event-forwarding
        - Add-command-execution-queue
        - Add-same-error-circuit-breaker
        - Add-same-error-circuit-breaker
        - Add-queue-status-merged
        - Improve-parallel-progress-responsiveness
        - Open-proposal-file-directly
        - Remove-hardcoded-main-branch
        - Add-worktree-view-with-merge
        - Add-operation-iteration-to-logs
        - Add-operation-iteration-to-logs (cleanup)
        - Add-operation-iteration-to-logs
        - Rename-to-conflux
        - Sync-tui-logs-to-debug-file
        - Sync-tui-logs-to-debug-file
        - Add-global-resolve-lock
        - Fix-parallel-apply-worktree-execution
        - Fix-streaming-retry
        - Fix-workspace-cleanup-guard
        - Fix-parallel-dynamic-queue-immediate-start
        - Add-tui-stop-cancel
        - Unify-ai-command-runner
        - Fix-web-dashboard-refresh
        - Refactor-execution-loop
        - Fix-merged-state-detection
        - Fix-parallel-command-queue
        - Fix-mergewait-resolve-running
        - Fix-mergewait-resolve-running
        - Fix-merged-state-detection
        - Fix-merged-state-detection
        - Add-worktree-setup-script
        - Add-worktree-setup-script
        - Fix-archive-verification-changes-remain
        - Fix-archive-verification-changes-remain
        - Fix-parallel-concurrency-limit
        - Fix-parallel-concurrency-limit
        - Avoid-merge-abort-conflict-check
        - Remove-cli-openspec-flags
        - Remove-cli-openspec-flags
        - Fix-worktree-archive-progress
        - Remove-archived-change-dir
        - Remove-archived-change-dir
        - Add-utc-build-number
        - Fix-merge-resolve-status
        - Fix-merge-resolve-status
        - Audit-parallel-reanalysis-gaps
        - Refactor-web-monitoring-parity
        - Refactor-web-monitoring-parity
        - Add-acceptance-loop
        - Add-acceptance-loop
        - Add-utc-build-number (manual cleanup - already merged)
        - Add-utc-build-number
        - Add-acceptance-integration
        - Fix-parallel-queue-reanalysis
        - Fix-parallel-queue-reanalysis
        - Acceptance-fail-apply-loop
        - Web-ui-status-align-and-summary
        - Fix-parallel-queue-reanalysis
        - Add-acceptance-continue-state
        - Add-acceptance-continue-state
        - Add-tui-accepting-status
        - Fix-order-based-slot-launch
        - Add-change-delta-conflict-check
        - Fix-tui-progress-fallback
        - Fix-tui-progress-fallback
        - Fix-merged-analysis-loop
        - Stall-merge-timeout-circuit-breaker
        - Add-log-headers-analysis-resolve
        - Fix-slot-availability-count
        - Add-acceptance-iteration-header
        - Add-web-api-openapi-docs
        - Add-web-api-openapi-docs
        - Refactor-reanalysis-trigger
        - Refactor-reanalysis-trigger
        - Add-merge-wait-auto-clear
        - Reanalysis-trigger-fix
        - Merge-wait-does-not-stop-orchestration
        - Tui-stopped-queue-policy
        - Fix-parallel-reanalysis-dispatch
        - Fix-parallel-reanalysis-dispatch
        - Fix-archive-tasks-fallback
        - Remove-parallel-batch-processing
        - Fix-parallel-cancel-worktree-cleanup
        - Fix-parallel-cancel-worktree-cleanup
        - Fix-parallel-cancel-worktree-cleanup
        - Fix-reanalysis-concurrent-dispatch
        - Fix-reanalysis-concurrent-dispatch
        - Fix-merge-tree-conflict-check
        - Fix-merge-tree-conflict-check
        - Fix-tui-archive-progress-zero
        - Fix-tui-archive-progress-zero
        - Fix-merge-wait-resolve-flow
        - Refactor-agent-module-split
        - Refactor-agent-module-split
        - Refactor-shared-progress-helpers
        - Refactor-shared-progress-helpers
        - Refactor-tui-state-events-split
        - Add-accepting-spinner
        - Refactor-vcs-git-commands-split
        - Refactor-vcs-git-commands-split
        - Refactor-vcs-git-commands-split
        - Remove-completed-status
        - Allow-worktree-delete-while-running
        - Fix-tui-accepting-stop-status
        - Fix-tui-accepting-stop-status
        - Implement-web-tui-status-labels
        - Refactor-parallel-module-split
        - Refactor-orchestrator-state
        - Refactor-serial-run-service
        - Add-web-ui-execution-controls
        - Add-on-merged-hook
        - Add-acceptance-git-clean-check
        - Add-acceptance-git-clean-check
        - Add-acceptance-tail-to-apply
        - Add-resolve-wait-status
        - Fix-parallel-on-merged-hook
        - Add-acceptance-base-diff
        - Add-tui-command-logs
        - Test-on-merged-hook
        - Test-on-merged-hook-parallel
        - Test-hook-fire
        - Test-hook-simple
        - Test-hook-simple

    ### Bug Fixes

        - Use interactive shell and inherit environment for agent commands
        - **parallel**: Ensure correct base revision and archive in merge
        - **workspace**: Initialize working copy after workspace creation
        - Remove all --ignore-working-copy flags from jj commands
        - Handle archived change resume flow
        - Harden parallel merge and selection flow
        - Show uncommitted badge only for queueable changes
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
        - Improve concurrent dispatch for re-analysis loop
        - Refactor re-analysis dispatch and tracking.
        - Capture acceptance failure tail output
        - Refactor order-based merge wait flow in parallel execution
        - Improve error handling and TUI state management
        - Surface parallel failures in TUI
        - **tui**: Count active changes in header
        - Update acceptance testing scenarios and prompts
        - Improve acceptance parsing and prompt to avoid code block false positives
        - 展開済みコマンドをTUI Logs Viewに表示
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
        - Add parallel command queue fix proposal
        - Improve tasks completion handling across files
        - Update parallel reanalysis order specifications
        - Improve specifications for parallel analysis prompt
        - Revamp project workflow and tech stack details
        - Update project structure and filenames for consistency
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
        - **refactor**: Add approved refactoring proposals for code organization
        - **tui**: Add QR code popup for web UI access and auto port assignment
        - **tui**: Add QR code popup for web UI access
        - **events**: Unify event system across serial and parallel modes
        - **openspec**: Approve child process cleanup and merge commit fixes
        - Add reliable cross-platform child process cleanup
        - **tui**: Add elapsed time tracking for parallel execution
        - Drop jj backend for parallel execution
        - Warn on dirty worktree in parallel mode
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
        - Implement REST API requirements for web dashboard refresh fix.
        - **web**: Refresh change progress from worktrees
        - Update default worktree base directory and configurations
        - Enhance update prompts and apply system enforcement
        - Improve progress event handling in executor.
        - Update TUI task progress retention logic
        - Update resolve prompt to disallow --no-verify option
        - Enhance archive merge guards and validations.
        - Implement Reliable Archive Tracking for Changes Directory in CLI
        - Update log headers with iteration in TUI architecture
        - Refactor command line interface specifications
        - Update archive state handling and task completion gates in openspec.
        - Implement logging for TUI Worktrees view Enter operations
        - Improve error handling and logging in Git workflow.
        - Refine archive state detection and task checks
        - Update TUI with merged progress refresh capability
        - Update parallel reanalysis order specifications.
        - Implement fix for merge resolve status
        - Implement acceptance loop for confirmation before archive.
        - Improve acceptance prompt functionality
        - Implement acceptance CONTINUE state handling across CLI.
        - Implement fix for parallel queue reanalysis.
        - Refactor web UI for improved status alignment and summary
        - Implement acceptance failure handling with apply retry loop
        - Update resume acceptance process in openspec
        - Update dependency analysis criteria documentation.
        - Update acceptance behavior to default CONTINUE
        - Normalize acceptance markers and tighten dependency analysis
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
        - Enhance merge-wait-resolve flow documentation
        - Update worktree cleanup policy documentation and design
        - Enhance merge conflict resolution and log auto-scroll logic
        - Update OpenSpec to remove 'completed' status across files
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
        - **openspec**: Add acceptance tail to apply change proposal
        - **openspec**: Add change proposal for TUI command logs display
        - Implement acceptance base diff feature across specs and tasks.

    ### Fix

        - Remove --ignore-working-copy from archive commits
        - Always create merge commits for individual changes

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

    ### Refactoring

        - Simplify orchestrator architecture and add openspec integration
        - **tui**: Replace ToggleApproval with mode-specific approval commands
        - **executor**: Replace blocking command execution with async streaming
        - **tui**: Simplify change state refresh logic
        - Refactor code for improved readability
        - Execute AI agent commands in workspace directory instead of repo root
        - Refactor and enhance parallel execution functionality
        - Enhance thread safety in environment locks
        - Implement parallel concurrency limit for workspace creation
        - Update archive verification logic and test coverage
        - Refactor TUI worktree merge eligibility specifications
        - Optimize parallel processing module
        - Update acceptance pass detection specifications
        - Remove parallel batch processing assumptions
        - Refactor module responsibilities for better maintainability
        - Refactor workspace cleanup behavior and legacy prompt
        - Refactor orchestrator state for shared model integration.
        - Update config default checks and guidelines
        - Update default configurations and regression tests
        - Refactor SerialRunService pattern into new service
        - Refactor error handling and method calls in executor and output modules
        - Refactor version string display in `utils.rs`
        - Optimize code for improved performance and readability
        - Refactor serial execution and update dependency statuses
        - Update running count and resolve slots in parallel execution.
        - **web**: Optimize CSS transitions and improve text wrapping

    ### Styling

        - Improve visibility by increasing Logs section height.

    ### Testing

        - Revert to zlnynurz to debug

    ### WIP

        - Fix-non-empty-merge-commits (19/24 tasks)
        - **archive**: Fix-tui-auto-refresh-pruning (attempt#1)
        - **archive**: Fix-worktree-archive-progress (attempt#1)
        - **archive**: Fix-worktree-archive-progress (attempt#2)
        - **archive**: Fix-worktree-archive-progress (attempt#3)
        - **archive**: Remove-archived-change-dir (attempt#1)
        - **archive**: Remove-archived-change-dir (attempt#2)
        - **archive**: Merge-wait-does-not-stop-orchestration (attempt#1)
        - **archive**: Merge-wait-does-not-stop-orchestration (attempt#2)

    ### Changeset

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
        - Fix-jj-conflict-detection
        - Fix-jj-conflict-detection
        - Fix-archive-failed-status
        - Refactor-parallel-executor
        - Grayout-archived-checkbox
        - Fix-stopped-mode-approval-queue
        - Skip-dependent-changes-on-error
        - Skip-dependent-changes-on-error
        - Skip-dependent-changes-on-error
        - Preserve-workspace-on-error

    ### Cleanup

        - Remove already-merged change fix-parallel-merge-completed-status

    ### Impl

        - Add loop history context for archive and resolve operations

    ### Spec

        - **parallel**: Define resolve goals and cleanup
        - **parallel**: Propose base-to-worktree presync before merges

    ### Temp

        - Resolve config
