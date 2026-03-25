## 1. Verdict parser robustness
- [x] 1.1 Change `parse_acceptance_output` to use `starts_with` instead of exact match for PASS/FAIL/CONTINUE/BLOCKED markers
- [x] 1.2 Make `strip_markdown_decorations` visible to `orchestration::acceptance` module (`pub(crate)`)
- [x] 1.3 Add regression tests for trailing-text cases (`PASSAll`, `FAILSome`, `CONTINUENeeds`, `BLOCKEDWaiting`)

## 2. Grace period after marker detection
- [x] 2.1 Implement marker detection in acceptance output streaming loop (`src/orchestration/acceptance.rs`)
- [x] 2.2 Apply 30-second grace period timeout after marker detection; terminate process on expiry
- [x] 2.3 Verify existing tests pass with the new loop structure

## 3. Acceptance marker formatting instructions
- [x] 3.1 Add CRITICAL formatting rule to `skills/cflx-workflow/SKILL.md` Output Format section
- [x] 3.2 Add CRITICAL formatting rule to `skills/cflx-workflow/references/cflx-accept.md`
- [x] 3.3 Add CRITICAL formatting rule to `.opencode/commands/cflx-accept.md`
