# Frontmatter line-ending contract

## One scanner, byte-preserving outputs

Create one internal scanner in `src/openspec.rs`, e.g. `scan_proposal_frontmatter`, with three logical outcomes: Absent, Present { yaml, body }, Unclosed. `yaml` and `body` borrow the input string. The names are local implementation details, but the three-way distinction is mandatory.

Read lines by byte offsets. A newline is LF with an optional immediately preceding CR. For delimiter comparison only, exclude that terminator. The first complete line must be exactly `---` with an LF/CRLF terminator; no BOM stripping or whitespace trimming. Search subsequent line spans for a line exactly `---`, also accepting the final EOF line. An immediate closing delimiter yields an empty YAML slice. Preserve all non-delimiter bytes, including CRLF within YAML scalar bodies and ordinary document body text. A final bare `---` without an opening line terminator keeps its existing no-frontmatter behavior; it is not an opened block.

Do not use `str::lines()` plus reconstruction, blanket newline substitution, or scanner output as the canonical Git content. Existing Git commit/blob/digest inputs stay unchanged.

## Consumer mapping

- Execution reader: Absent => existing legitimate empty source; Present => current typed YAML decoder; Unclosed => proposal-scoped metadata-invalid defect with the current bounded value-free detail.
- Strict frontmatter decoder: Present => existing decoder; Unclosed => a bounded structural diagnostic, never a decoder error echo; Absent => current empty metadata result. Strict validation still uses the execution reader's typed diagnostics when available.
- Display reader: Present => tolerant current metadata decoder; Absent/Unclosed => its existing display fallback. Display fallback is not execution authority.
- Strict dependency reader: Unclosed => current parse error; Present => current strict metadata/dependency handling; Absent => existing body dependency fallback.
- Task linkage, change-type lookup, release-dependency validation and manifest building already consume these shared paths; audit them for independent delimiter checks and remove only relevant redundant LF-only checks.

## Invalid versus empty

After a recognized opening delimiter, lack of a recognized closing line is invalid even with CRLF or mixed terminators. Bad YAML and wrong-typed fields keep the existing typed defect/identity. An empty block, null verification value, empty list and absent frontmatter remain legitimate empty sources. This does not remove separate implementation-proposal requirements enforced by strict validation.

## Evidence and tests

Use a deterministic gate that writes a count outside the target worktree and then exits with a selected code. The CRLF failing command is the important discriminator: a no-op parser selects zero and would incorrectly pass. The positive command proves there is no indiscriminate CRLF refusal. Compare normalized parsed declarations and typed keys, but assert that raw proposal/blob IDs are still distinct for different byte sequences. Native tests must mutate the candidate proposal bytes before manifest capture, not feed constructed gate plans. Evidence-store sentinels must be read back unchanged for invalid sources.

All fixtures are disposable and isolated. No live owner, real repository proposal, global Git configuration, credentials, external network or shared binaries are changed by the tests. Preserve old failure output when a fixture itself is corrected. This change does not certify the complete product suite or the original incident workflow.
