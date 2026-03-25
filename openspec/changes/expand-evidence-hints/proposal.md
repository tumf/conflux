# Expand _EVIDENCE_HINTS for Non-Python Ecosystems

## Problem / Context

`cflx.py` validation with `--evidence error` rejects valid verification notes in Node.js, Rust, and Go projects because `_EVIDENCE_HINTS` lacks common non-Python ecosystem patterns.

A verification note like:

```
(verification: run `npm run typeorm migration:run` against local DB -- column exists in table)
```

is rejected because none of the current hints match. The `.ts` hint catches most Node.js cases incidentally (when a `.ts` file path appears), but verification commands that don't reference a specific file extension fail.

**GitHub Issue**: #2

## Proposed Solution

Add 12 new hint strings to `OpenSpecManager._EVIDENCE_HINTS` in `skills/cflx-proposal/scripts/cflx.py` to cover Node.js, Rust, and Go ecosystem patterns:

| Hint | Rationale |
|---|---|
| `"npm test"` | Standard Node.js test runner |
| `"npm run "` | Standard Node.js script runner (trailing space prevents false match on `npm running`) |
| `"npx "` | Node.js package executor |
| `"yarn "` | Yarn package manager |
| `"pnpm "` | pnpm package manager |
| `"cargo test"` | Rust test runner |
| `"cargo build"` | Rust build command |
| `"go test"` | Go test runner |
| `"test/"` | Common test directory (complements existing `"tests/"`, common in JS/Ruby) |
| `".spec"` | Spec file pattern (e.g., `foo.spec.ts`) |
| `".test"` | Test file pattern (e.g., `foo.test.ts`) |

### Excluded from Issue suggestion

- `"grep "` — Not repository-verifiable evidence in the intended sense; dynamic output inspection is non-idempotent and would increase false positives.

## Acceptance Criteria

1. Verification notes containing `npm run`, `npm test`, `npx`, `yarn`, `pnpm`, `cargo test`, `cargo build`, `go test`, `test/`, `.spec`, or `.test` pass `--evidence error` validation
2. Existing validation behavior is unchanged (additive-only change)
3. `cflx.py validate <id> --strict --evidence error` passes for the reproduction case in Issue #2

## Out of Scope

- Adding unit tests for `_has_repository_evidence_hint` (separate concern)
- Changes to `cflx-workflow/scripts/cflx.py` (no evidence feature there)
- Changes to the matching logic (`any(hint in normalized)`)
