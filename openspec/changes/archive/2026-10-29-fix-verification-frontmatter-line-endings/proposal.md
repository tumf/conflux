---
change_type: implementation
priority: high
dependencies:
  - fix-verification-declaration-integrity
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/proposal-metadata/spec.md
  - src/openspec.rs
  - src/openspec_cmd/validation.rs
  - src/openspec_cmd/verify.rs
  - src/orchestration/acceptance/manifest_builder.rs
  - src/parallel/tests/acceptance_execution_boundary.rs
  - tests/run_exit_tests.rs
verifications:
  - id: frontmatter-line-endings-native
    requirement: LF and CRLF declarations have identical selection and fail-closed native behavior, including after Apply changes the candidate bytes
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/openspec.rs
    evidence: cargo test --locked --offline --lib --features web-monitoring,heavy-tests verification_frontmatter_line_endings -- --nocapture
    rerun: cargo test --locked --offline --lib --features web-monitoring,heavy-tests verification_frontmatter_line_endings -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: frontmatter-line-endings-cli
    requirement: The real CLI selects and executes the same gate for LF, CRLF and mixed supported line endings, and defective sources fail before evidence access
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: cargo test --locked --offline --test run_exit_tests --features web-monitoring,heavy-tests verification_frontmatter_line_endings -- --nocapture
    rerun: cargo test --locked --offline --test run_exit_tests --features web-monitoring,heavy-tests verification_frontmatter_line_endings -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Preserve required verifications across frontmatter line endings

**Change Type**: implementation

## Problem

Both `verification_frontmatter` and the display/strict `split_frontmatter` in `src/openspec.rs` require the literal opening bytes `---\n`. A proposal written with CRLF therefore becomes an apparently absent frontmatter block. The execution-authoritative reader returns a valid empty source, so `verify --plan --json` and `verify --json` exit zero with no verifications even when the declared command would fail. Strict validation can reject the implementation proposal for missing evidence, but that does not certify a candidate whose proposal changes after Apply. The existing LF type-error correction does not close this different boundary.

## Selected solution

Use one byte-preserving delimiter scanner for execution, strict/display metadata and strict dependency reading. Recognize the opening line `---` at byte offset zero with LF or CRLF, and a closing line exactly `---` with LF, CRLF, or end of file. LF/CRLF may be mixed per line. The scanner distinguishes absent frontmatter, present frontmatter (including an empty/null block), and an opened but unclosed block. It returns slices into the original string: do not rewrite files, normalize Git blobs, trim YAML, or normalize digest inputs. YAML decoding remains with the existing serde reader and the existing typed defect/key machinery.

The new scanner must replace delimiter decisions in both existing splitters and the strict dependency reader; do not merely add a second CRLF-only execution path. Preserve each caller's error contract while preventing line endings from turning an opened or valid declaration block into `absent`.

## Change boundary and preserved contracts

- Product changes are limited to `src/openspec.rs` and directly necessary consumers in `src/openspec_cmd/{validation,verify}.rs` and `src/orchestration/acceptance/manifest_builder.rs`. Add focused native-boundary tests in the existing Acceptance/executor test modules and real CLI tests in `tests/run_exit_tests.rs`; update proposal-metadata specification/docs only for this contract.
- Consume the integrated typed `VerificationSource`, `DeclarationKey`, defect propagation and constructor from `fix-verification-declaration-integrity`; this is a code dependency, not operational ordering.
- Preserve legitimately empty sources, non-required declaration exclusion, source-unavailable handling, command syntax and leading environment assignments, repair identity, reviewer verdict semantics, API schemas, and retry/admission routing.
- Candidate commit/blob/digest identities remain over actual original bytes; LF and CRLF can decode equivalently while retaining different content identities.
- No new flag, option, environment variable, LF-only authoring restriction, implicit shell, durable workflow state, or target-worktree runtime artifact is permitted.

## Acceptance and explicit completion

1. Parser and metadata parity: LF, CRLF and mixed LF/CRLF versions of the same complete proposal expose the same metadata, dependencies, ordered declarations and typed defect keys. Closing delimiter at EOF and empty blocks work with either opening terminator. Empty/Null roots yield empty strict/display metadata without decode diagnostics; strict dependencies retain body fallback, while implementation-required verification policy remains enforced. YAML slices end at the closing line start offset, preserving the complete preceding LF/CRLF terminator. An opened CRLF block without a closing delimiter is `declaration_metadata_invalid`, never absent. A line with an isolated carriage return is not a supported delimiter. BOM handling and leading whitespace outside the established contract are unchanged.
2. Real CLI: use disposable tracked repositories and a non-echoing local gate. In both LF and CRLF fixtures, a passing gate selects exactly one verification and succeeds; a failing gate selects exactly one and returns a nonzero actual run. The plan may succeed without running the failing command but must select it. Strict and archive-gate validate the valid declaration independently of its future runtime exit. Both validators and verify plan/run reject wrong-typed and unclosed sources; verify must not execute a marker gate or read/write existing evidence on invalid input. Keep a no-frontmatter/empty source control for verify.
3. Native candidate boundary: start with a valid LF proposal at admission, then exercise the production manifest/gate boundary using the candidate's CRLF bytes after Apply. A failing required command must execute, fail natively and prevent reviewer start; a passing CRLF gate executes once before one reviewer invocation. CRLF wrong-typed or unclosed metadata must produce typed defects, with no gate, reviewer, evidence reuse or write. Tests must use the production reader/manifest path, not a hand-built empty/defective manifest.
4. Every new test name contains `verification_frontmatter_line_endings`. Both declared commands must execute nonzero tests. Existing declaration integrity, leading-env and proposal metadata tests must remain green; report their exact counts and any unrelated suite failures separately. No test may assert success merely from matching labels, a no-op fake reader, or validator exit without the gate marker/count.
5. No product changes outside the declared scope; all proposal artifacts agree with this contract. Strict/archive validation is structural evidence, not implementation acceptance.

## Rejected alternatives and non-goals

- Asking users to convert every file to LF or adding an opt-in switch would preserve the silent omission defect and is rejected.
- Globally replacing CRLF bytes before parsing/hashing risks content-identity drift and is rejected; scan delimiters and retain slices instead.
- Executing shell syntax, weakening the gate planner, broad parser/logging refactors, hook-liveness repair, unrelated full-suite test fixes, Web/TUI design changes, push, release, installation and replacing a live owner are out of scope.
- The existing independent runtime acceptance may continue, but does not certify this later correction. Do not touch the original `scope-drill` candidate or the original main-based owner.

## Verification ownership

Parser/metadata controls are unit tests; native manifest-to-gate behavior is integration-scoped inside the lib tests; real `cflx` invocation is covered by the integration target. The tracked pre-commit config has path-filtered Rust hooks, not an unconditional acceptance gate, so it does not replace the explicit requirement-specific commands. Runtime acceptance and any deployment are separate observations, not checkbox completion from this proposal.
