# Design

## Single command authority

Heavyweight detection reads only `verifications[].evidence` and `verifications[].rerun`. Task notes continue to link checkboxes to verification IDs but do not duplicate command authority and are not parsed for marker or command cohesion.

## Token matching

Commands are split on ASCII whitespace after removing Markdown backticks, and each token is trimmed of surrounding shell punctuation (`$ ( ) { } [ ] ; | & < > " ' ,`) so `$(seq` and `3);` compare as `seq` and `3`. Matching is case-insensitive and exact by token, except the explicit `qemu-system-*` executable prefix. Multi-token forms such as `docker compose`, `docker run`, `cargo bench`, and `--features heavy` require adjacent token pairs; `--features=heavy` is carried as its own exact token. Substring containment is forbidden.

`docker` alone is never a match: only the orchestration subcommands `compose`, `run`, and `swarm` pair with it, which is what keeps a bounded `docker build` valid without a special case.

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

### Method

The survey is produced by the shipped matcher, not by a re-implementation of it: `cflx openspec validate <id> --strict` is run from a `cargo build --bin cflx` binary of this change and its `heavyweight command form` warnings are collected. The archived corpus is surveyed the same way, by copying `openspec/changes/archive/*` into a scratch workspace alongside a copy of `openspec/specs` and running one `cflx openspec validate --strict` over all of them.

### Active proposals (authoritative, 2026-08-23)

Two active proposals exist; both declare change-blocking verifications, and **both produce zero warnings**.

| Proposal | Change-blocking `evidence` / `rerun` | Result |
| --- | --- | --- |
| `correct-acceptance-runtime-routing` | `cargo test parallel:: --lib`, `cargo test config:: --lib` (both fields, both declarations) | no match: no denied token; `parallel::` and `config::` are narrow filters |
| `refine-verification-gate-policy` | `cargo test openspec_cmd --lib` (both fields) | no match: no denied token |

Warning count on the active set: **0 of 2 proposals, 0 of 3 change-blocking declarations.** Hard-error promotion would therefore be a no-op against today's active set, which is exactly why the promotion decision needs the wider corpus below rather than this table alone.

### Archived corpus (supplementary, 97 change-blocking proposals)

| Result | Proposals |
| --- | --- |
| Warned | 15 |
| Clean | 82 |

Every warning is the same match: `broad selector: '--all-features'`, in `rerun`, never in `evidence`. Two populations hide behind that single token:

- **3 true positives** — `--all-features` is a *test* selector: `2026-08-02-classify-external-blockers` (`cargo test --all-features && ...`), `2026-08-05-allow-tui-ahead-worktree-discard` (`cargo test --all-features`), `2026-08-05-show-workspace-preparing-status` (`cargo test --all-features preparing`).
- **12 probable false positives** — `--all-features` appears only inside a trailing `cargo clippy --all-targets --all-features -- -d warnings` lint step chained after focused tests: `2026-07-31-fix-idle-parallel-stop-classification`, `2026-08-04-allow-tui-dirty-worktree-delete`, `2026-08-06-fix-force-stop-reducer-reconciliation`, `2026-08-06-fix-precomplete-apply-repair-termination`, `2026-08-06-fix-run-owned-process-cleanup`, `2026-08-06-preserve-external-blocker-metadata`, `2026-08-06-retry-transient-queue-classification`, `2026-08-06-show-ready-header-after-stop`, `2026-08-07-restore-ready-on-persistent-idle`, `2026-08-07-synchronize-execution-marks`, `2026-08-08-avoid-global-stop-on-merge-wait`, `2026-08-13-add-cflx-client-mcp`. A repository-wide clippy pass is bounded and is not the broad *suite* the rule targets.

### What exact-token matching already prevented

- `--features heavy-tests` occurs in **13** archived change-blocking declarations and correctly does not match `--features heavy`. A substring matcher would have reported all 13.
- The bare word `seq` occurs as a substring of prose evidence ("event sequences", "command sequences") in **2** declarations, and `bench` inside "benchmark output" in **1**. None match.
- No archived change-blocking declaration names `docker`, `podman`, `kubectl`, `qemu-system-*`, `xargs`, `--workspace`, `--ignored`, `--include-ignored`, or `--exhaustive`, so those classes have no observed evidence in either direction yet.

### Known false negatives in this corpus

- Bare `cargo test` (whole-repository suite) in **2** declarations.
- `--features heavy-tests`, which is this repository's own heavy-suite gate, in **13** declarations.
- `make <target>` wrappers (`make check-openapi`, `make web-test`) in **12** declarations, whose contents the matcher cannot see.
- `for <var> in <list>; do ...; done` repetition without `seq`/`xargs` in **3** declarations.

### Promotion preconditions

A follow-up proposal that promotes any class to an error must re-run this survey and must, at minimum, resolve the `--all-features` split above — either by narrowing the token to a test-command context or by dropping it — because on this corpus it is wrong about four declarations out of five.

## Runtime boundary

The validator does not execute `evidence` or `rerun` and cannot prevent an AI session from independently choosing a heavy command. It removes or warns about declared authorization; bundled guidance remains responsible for agent behavior.
