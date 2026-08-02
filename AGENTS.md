# AGENTS.md - Conflux

Essential information for AI coding agents working on this Rust codebase.

## Project Overview

Conflux(cflx) automates the OpenSpec change workflow (list → dependency analysis → apply → acceptance → archive → resolve → merged). It orchestrates `openspec` and AI coding agent tools to process changes autonomously.

## Self-hosted Development

* Find cflx logs: `~/.local/state/cflx/logs/conflux-{slug}/YYYY-MM-DD.log`

## Frontends
Conflux has the following frontends:

* TUI
* WebUI (local `--web` monitoring)

## Web UI

The WebUI is an optional local monitoring dashboard enabled with `--web` on
`cflx`, `cflx tui`, or `cflx run`. There is no standalone server daemon and no
multi-project mode.

The operator console consists of `web/index.html`, `web/style.css`, and
`web/app.js`, embedded via `include_str!` in `src/web/mod.rs`. No build step and
no frontend framework: it is dependency-free HTML/CSS/JS and a first-class
`/api/v2` client. There is no legacy unversioned `/api/*` or `/ws` surface any
more. Browser tests live in `tests/web/` and run with `make web-test`; that
tooling is dev-only and must never become a production dependency.

## Directories

* `src/`: Main Rust source code
* `tests/`: Rust test code
* `web/`: Embedded static web assets used by the WebUI
* `skills/`: Source files for `cflx-*` skills that are embedded into the Rust binary
* `openspec/`: OpenSpec changes and specs
* `docs/`: Project documentation
* `scripts/`: Development and release helper scripts

## Serial or Parallel Mode

* Parallel mode: Mainly used
* Serial mode: Obsolete (to be removed)

## Constitution

* `openspec/CONSTITUTION.md` が存在する場合、proposal・spec・implementation より上位の規範として必ず従うこと。
* 憲法レベルの原則を変更する場合は、`openspec/CONSTITUTION.md` 自体を同じ change で明示的に更新すること。

## Skills

It also depends on `cflx-*` skills developed under the `skills/` directory.
The skill files are embedded into the Rust binary via `include_str!` at compile time.
**NEVER EDIT** `~/.agents/skills/cflx-*` skills. These will be overwritten by `cflx install-skills --global`.

## Unit Tests

Tests taking over 1 second must either be optimized to run in under 1 second or, if that is not practical, marked with `#[cfg_attr(not(feature = "heavy"), ignore)]`. Heavy tests must not run as part of the default test suite.

Use `bd` for task tracking.
