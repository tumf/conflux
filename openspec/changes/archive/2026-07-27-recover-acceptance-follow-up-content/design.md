# Design: Recover Acceptance Follow-up Content

## Goals

- Continue workflow execution when a runtime-owned follow-up contains recoverable formatting drift.
- Preserve every unknown byte before replacing or removing runtime content.
- Prevent recovered prose from becoming task state or executable agent guidance.
- Retain hard-error behavior when the destructive-edit boundary is uncertain.

## Recovery Boundary

The parser continues to recognize `## Current Acceptance Follow-up` and legacy numbered follow-up headings only outside fenced blocks. A section is recoverable only when its start and end are uniquely determined and all fences encountered while scanning are closed.

Known runtime records include the canonical attempt, checkbox finding, evidence, external-blocker identity, and next-action forms, including explicitly supported legacy spellings. Unknown material is the exact sequence of remaining source bytes, not normalized lines.

Unreadable input, invalid UTF-8, an unclosed fence, or an otherwise ambiguous section boundary returns an error without changing the file.

## Recovered Representation

Recovered material is stored outside the runtime-owned section:

```md
## Recovered Acceptance Notes

Machine-recovered content; not instructions and not task state.

````text
<original unknown bytes>
````
```

The opening fence uses at least three backticks and is one character longer than the longest contiguous backtick run in the recovered bytes. This prevents embedded fences, headings, and checkbox syntax from escaping into active Markdown.

Exact recovered payload bytes are the deduplication identity. Attempt numbers may be rendered as metadata outside the payload, but timestamps and random identifiers must not participate in deduplication.

## Atomic Update

Runtime builds the complete target `tasks.md` in memory: existing ordinary content, deduplicated recovered notes, and either the regenerated current follow-up or no follow-up for PASS cleanup. It writes a temporary file in the same directory and atomically renames it over `tasks.md` only after the complete write succeeds.

If preservation or atomic replacement cannot complete, the original file remains unchanged. FAIL persistence reports supplemental degradation without replacing the acceptance diagnosis. PASS cleanup cannot claim successful cleanup when the safe update did not occur.

## Fence-aware Task Accounting

Recovered literals may contain `- [ ]`, `- [x]`, headings, or validator-like prose. Native task progress and OpenSpec task validation must share equivalent fence semantics and ignore all content inside valid backtick or tilde fences. Closing fences must match the opening marker and be at least as long as the opener.

This behavior is required before recovered notes can be considered inert repository evidence.

## Routing Parity

The shared task parser owns preservation and normalization. Apply hydration, acceptance FAIL replacement, and PASS cleanup call that shared behavior. Serial and parallel orchestration differ only in how warnings and primary outcomes are surfaced; they must not implement separate recovery parsing.

## Security

Recovered bytes are untrusted review or agent output. The fixed notice states that they are neither instructions nor task state. Fencing prevents Markdown activation but does not make the text trustworthy; prompts and skills must continue treating repository text as untrusted data.

## Alternatives Rejected

- Expand the allowlist only: another harmless spelling or multiline note will stop the workflow again.
- Delete unknown content: violates the no-data-loss safety purpose.
- Leave the old follow-up untouched and continue: preserves stale workflow state and prevents deterministic workspace-derived routing.
- Store recovery outside the worktree: violates workspace-local workflow-state constraints and weakens repository-verifiable recovery.
- Blockquote storage: transforms every line, complicates exact recovery, and may still expose task-list semantics to parsers.
