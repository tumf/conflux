## Implementation Tasks

- [ ] 1. Audit workflow-split prompt ownership across acceptance, analyze, resolve, apply, archive, cleanup-review, and rejecting so each operation's fixed guidance source and runtime-context boundary are explicitly documented in repo-facing artifacts (verification: manual/spec - proposal/design/spec deltas cite the authoritative source for each operation and identify any intentional compatibility exceptions).
- [ ] 2. Tighten the acceptance output contract in `.opencode/commands/cflx-accept.md`, relevant skill guidance, and canonical specs so the final verdict marker is machine-readable and unambiguous, including explicit treatment of markdown heading / quote / bullet / fenced output forms (verification: unit/spec - prompt/contract tests and `cflx openspec validate harden-workflow-split-prompt-contracts --strict` pass with the updated contract).
- [ ] 3. Update `src/acceptance.rs` and any acceptance runtime parsing path so verdict handling matches the documented contract and no longer silently degrades into `CONTINUE` for the accepted set of verdict encodings (verification: unit - targeted parser tests cover plain marker output plus drift cases observed in production-style logs).
- [ ] 4. Add regression tests for workflow split ownership drift so dedicated skills / command templates / Rust prompt builders fail fast when fixed guidance or output contracts are duplicated, omitted, or contradicted after future refactors (verification: unit/integration - prompt-builder and embedded-skill tests fail on drift phrases and pass when ownership boundaries remain intact).
- [ ] 5. Run focused repository checks for the touched prompt, parser, and spec surfaces before handoff (verification: unit/integration - targeted Rust tests for acceptance parsing/prompt assembly plus strict OpenSpec validation all pass).

## Future Work

- Extend the same ownership-drift audit pattern to any future operation-specific prompts introduced after the current workflow split surface
