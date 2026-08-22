# Design

## Single command authority

Heavyweight detection reads only `verifications[].evidence` and `verifications[].rerun`. Task notes continue to link checkboxes to verification IDs but do not duplicate command authority and are not parsed for marker or command cohesion.

## Token matching

Commands are split on ASCII whitespace after removing Markdown backticks. Matching is case-insensitive and exact by token, except the explicit `qemu-system-*` executable prefix. Multi-token forms such as `docker compose`, `docker run`, `cargo bench`, and `--features heavy` require adjacent token pairs. Substring containment is forbidden.

Allowed boundary examples:

- `docker build .`
- `cargo test full_pipeline_smoke --lib`
- `cargo test benchmark_parser_units --lib`

Warning examples:

- `docker compose up --wait`
- `docker run image`
- `cargo bench`
- `cargo test --workspace`
- `cargo test --features heavy`
- `for i in $(seq 3); do cargo test; done`

The last example is detected from structural `seq`; prose such as “run three times” is not parsed.

## Migration

The first release emits warnings in strict validation and does not fail archive-gate validation. Legacy declarations for which structured verification linkage is unavailable remain inert. Hard-error promotion requires a follow-up proposal with a fresh active-proposal survey and observed false-positive/false-negative evidence.

## Active-proposal survey

Implementation MUST enumerate active `openspec/changes/*/proposal.md` declarations and record the count and per-proposal reasons produced by the candidate token matcher. This section is completed with those results during Apply and is required before any future error promotion.

## Runtime boundary

The validator does not execute `evidence` or `rerun` and cannot prevent an AI session from independently choosing a heavy command. It removes or warns about declared authorization; bundled guidance remains responsible for agent behavior.
