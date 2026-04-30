# AGENTS.md - Conflux

Essential information for AI coding agents working on this Rust codebase.

## Project Overview

Conflux(cflx) automates the OpenSpec change workflow (list → dependency analysis → apply → acceptance → archive → resolve → merged). It orchestrates `openspec` and AI coding agent tools to process changes autonomously.

## Self-hosted Development

* Find cflx logs: `~/.local/state/cflx/logs/conflux-{slug}/YYYY-MM-DD.log`

## Frontends
Conflux has the following frontends:

* TUI
* WebUI (server mode)

## Web UI

The WebUI provides a dashboard when Conflux runs in server mode.
The dashboard source files are located in the `dashboard/` directory.
The build output (`dashboard/dist/`) is embedded into the Rust binary via `include_str!` at compile time.

## Directories

* `src/`: Main Rust source code
* `tests/`: Rust test code
* `dashboard/`: WebUI dashboard source files
* `web/`: Embedded static web assets used by the Rust application
* `skills/`: Source files for `cflx-*` skills that are embedded into the Rust binary
* `openspec/`: OpenSpec changes and specs
* `docs/`: Project documentation
* `scripts/`: Development and release helper scripts

## Serial or Parallel Mode

* Parallel mode: Mainly used
* Serial mode: Obsolete (to be removed)

## Workspace State Principle

* Workflow state must be derivable from the workspace alone.
* Do not introduce or depend on out-of-worktree durable workflow state for resume routing, acceptance gating, or archive routing.
* Deleting `~/.local/state/cflx/**` must not change the next action chosen for the same workspace contents.
* External logs, metrics, or UI caches are allowed only as non-authoritative observability outputs and must never be used as workflow control inputs.

## Skills

It also depends on `cflx-*` skills developed under the `skills/` directory.
The skill files are embedded into the Rust binary via `include_str!` at compile time.
**NEVER EDIT** `~/.agents/skills/cflx-*` skills. These will be overwritten by `cflx install-skills --global`.

## Graphify

This repository keeps a graphify knowledge graph in `graphify-out/`.

- Before answering architecture or cross-module questions, read `graphify-out/GRAPH_REPORT.md` first.
- If `graphify-out/wiki/index.md` exists, prefer navigating it instead of reading raw files.
- For cross-module "how does X relate to Y" questions, prefer `graphify query "<question>"`, `graphify path "<A>" "<B>"`, or `graphify explain "<concept>"` over grep.
- Do not run `graphify update .` on every intermediate code edit.
- Run `graphify update .` only when the final repository state that will land on main has been finalized, and include the resulting `graphify-out/` changes in that same final commit when they changed.

## Unit Tests

Tests taking over 1 second must either be optimized to run in under 1 second or, if that is not practical, marked with `#[cfg_attr(not(feature = "heavy"), ignore)]`. Heavy tests must not run as part of the default test suite.
