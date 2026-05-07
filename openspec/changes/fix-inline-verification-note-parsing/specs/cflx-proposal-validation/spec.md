## MODIFIED Requirements

### Requirement: evidence-hint-matching

The `_EVIDENCE_HINTS` tuple used by `_has_repository_evidence_hint` must include hints for common build/test toolchains across Python, Node.js, Rust, and Go ecosystems so that verification notes citing runnable commands from these ecosystems are accepted. Inline verification annotations must be recognized when they appear anywhere in a checkbox task line, including before additional completion-condition prose.

#### Scenario: Node.js npm run command accepted

**Given**: A task with verification note `(verification: run npm run build -- succeeds)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted (no error about missing repository-verifiable evidence)

#### Scenario: Rust cargo test command accepted

**Given**: A task with verification note `(verification: cargo test passes)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted

#### Scenario: Go test command accepted

**Given**: A task with verification note `(verification: go test ./... passes)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted

#### Scenario: Test directory path accepted

**Given**: A task with verification note `(verification: test/integration/auth.test.ts passes)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted

#### Scenario: Existing Python hints still accepted

**Given**: A task with verification note `(verification: pytest tests/test_auth.py passes)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted (backward compatible)

#### Scenario: Inline verification before completion prose accepted

**Given**: A behavior-bearing task line contains `(verification: manual - inspect src/openspec_cmd.rs and run cargo test openspec_cmd --lib)` followed by additional completion-condition prose on the same line
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: the verification note is accepted
**And**: validation does not report `Behavior-bearing task missing '(verification: ...)' note` for that line
