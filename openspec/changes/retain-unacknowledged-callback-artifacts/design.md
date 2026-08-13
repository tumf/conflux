# Design

## Explicit artifact ownership

`tempfile::TempDir` makes registry destruction an implicit cleanup authority. Preserve its randomized exclusive creation, then call `tempfile::TempDir::keep()` and store the resulting `PathBuf`. Keep the existing `restrict(path, 0o700)` check. Do not replace this with a predictable path or `create_dir_all`, which would permit shared-`TMPDIR` pre-creation and symlink attacks.

Positive dispatcher acknowledgement is exactly `Ok(Ok(()))` from the shutdown wait and is the only state that calls `remove_dir_all`. A sender dropped before the shutdown deadline currently appears as `Ok(Err(RecvError))`; sender drop after cancellation appears as `Err` from the awaited receiver. Both, plus task-send failure, retain artifacts and emit one bounded `warn!` containing only the directory path. No payload, token, environment value, or callback output enters the warning.

The regression needs no public abort hook. Start the registry and callback on Tokio runtime A, prove the callback started from its own log, then drop runtime A so the dispatcher receiver and queued `Task::Stopping` acknowledgement sender disappear. Drive `owner_stopping()` from runtime B and assert that the pre-deadline sender-drop path returns without deleting the event file or directory. A public dispatcher kill/abort hook is prohibited because it would create a production API that can silently disable delivery.

The retained directory is non-authoritative observability data and never participates in workflow routing, so this does not add durable workflow state.
