# Design: Retry Archive Commit Finalization

## Current Architecture

Parallel archive currently has two different retry surfaces:

1. `src/parallel/executor.rs::execute_archive_in_workspace` runs the configured archive command and retries when `verify_archive_completion()` says the active change still exists or archive files were not created.
2. After archive move verification succeeds, `src/execution/archive.rs::ensure_archive_commit` tries to create the final `Archive: <change_id>` commit.

The first surface is looped. The second is effectively one direct commit attempt plus one AI resolve attempt. A hook/clippy/format failure during final commit creation can therefore terminate the run even though the archive files are in the right place and the error is repairable.

## Target Architecture

`ensure_archive_commit` should own a small bounded finalization loop. Each iteration should:

1. Check whether `is_archive_commit_complete(change_id, Some(repo_root))` is already true.
2. Capture repository state relevant to finalization, including `git status --porcelain`, latest commit subject, archive path existence, and active change path absence.
3. If there are changes, attempt `git add -A` and `git commit -m "Archive: <change_id>"`.
4. If direct commit fails, preserve stderr/stdout and invoke the resolve agent with the previous blocker context.
5. After resolve exits, verify archive commit completion again.
6. Retry until success or finalization retry budget exhaustion.

## Retry Budget

The initial implementation may reuse the existing archive retry constant or introduce a dedicated private constant for archive commit finalization. The budget must be bounded. Runtime attempt counters are in-memory only and must not become durable workflow state.

## Context Passed to Agent

Subsequent finalization resolve attempts should include:

- last direct commit stderr/stdout
- last resolve command exit status and output tail
- current `git status --porcelain`
- whether the active change directory still exists
- whether the archive entry exists
- whether latest commit subject already starts with `Archive: <change_id>` or `WIP(archive): <change_id>`

This context lets the agent repair concrete hook failures instead of guessing.

## Event Semantics

Archive command retry and archive commit finalization retry are different phases. Logs/events should distinguish them so users can see whether Conflux is still trying to move files or only trying to finalize the archive commit.

## Constitution Alignment

The proposal does not introduce durable workflow-control state. The finalization loop uses workspace file state, workspace git state, base/archive verification, and in-memory attempt context, which is compatible with the constitution.
