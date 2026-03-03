## Implementation Tasks

- [x] Update tool_use summarization to always include a file path for `read`/`write`/`edit` when present (verification: unit tests assert `filePath=...` or a supported alias appears in the summary).
- [x] Ensure `write`/`edit` tool_use summaries never include raw body content (e.g., must not log `text=...`) (verification: unit tests assert the summary does not contain the body even if the input includes it).
- [x] (Optional) Add safe write/edit metadata such as `chars=<n>` and/or `lines=<n>` (verification: unit tests confirm metadata is present and no content is leaked).
- [x] Add/adjust regression tests for stream-json tool_use summary extraction (verification: `cargo test` passes).
- [x] Run strict OpenSpec validation for this change (verification: `openspec validate update-tool-use-filepath-logging --strict --no-interactive`).

## Future Work

- Decide whether and how to normalize file paths in summaries (repo-relative vs absolute) and propose a dedicated change if needed.
