# Design

## Explicit artifact ownership

`tempfile::TempDir` makes registry destruction an implicit cleanup authority. Store a `PathBuf` instead and create the directory with owner-only permissions. Positive dispatcher acknowledgement is the only path that calls recursive deletion. Missing acknowledgement intentionally leaks a bounded owner-private temporary directory rather than risking removal beneath a live callback.

The retained directory is non-authoritative observability data and never participates in workflow routing, so this does not add durable workflow state.
