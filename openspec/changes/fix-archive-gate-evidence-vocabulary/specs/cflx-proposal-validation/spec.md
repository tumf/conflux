## MODIFIED Requirements

### Requirement: evidence-hint-matching

The OpenSpec archive gate MUST evaluate repository-verifiable evidence and ownership markers from the complete task verification note, not from a truncated substring caused by parenthesized or backticked command/prose content inside the note. The evidence matcher MUST accept generic repository-evidence vocabulary used by Conflux diagnostics and proposal guidance when it appears with a valid verification ownership marker, including source-path, test-file, and runnable-command wording. The matcher MUST also accept common concrete repository artifact and build-command evidence such as Dockerfiles, TOML configuration files, and Docker build commands. Weak narrative notes without concrete repository evidence MUST remain rejected.

#### Scenario: Manual verification note contains source paths and runnable command

**Given**: an implementation change task line contains an inline `(verification: manual - ...)` note with source paths and a runnable `cflx openspec validate <id> --strict` command
**When**: `cflx openspec validate <id> --archive-gate` evaluates the task
**Then**: the archive gate accepts the task's verification evidence instead of reporting that repository-verifiable evidence is missing

#### Scenario: Verification note contains generic evidence vocabulary

**Given**: an implementation change task line contains a verification note with a valid ownership marker and generic repository evidence wording such as `source paths`, `test files`, or `runnable command`
**When**: `cflx openspec validate <id> --archive-gate` evaluates the task
**Then**: the archive gate accepts the note as repository-verifiable evidence

#### Scenario: Verification note contains common build artifacts and commands

**Given**: an implementation change task line contains a verification note with a valid ownership marker and evidence such as `Dockerfile`, `.toml`, or `docker build`
**When**: `cflx openspec validate <id> --archive-gate` evaluates the task
**Then**: the archive gate accepts the note as repository-verifiable evidence

#### Scenario: Verification note contains parenthesized command or prose content

**Given**: an inline verification note contains parenthesized or backticked command/prose segments before the repository evidence hint
**When**: the validator extracts the verification note
**Then**: extraction includes the full evidence-bearing note rather than stopping at the first inner closing parenthesis

#### Scenario: Missing or weak verification remains rejected

**Given**: an implementation change task has no verification note, lacks a recognized verification ownership marker, or lacks repository-verifiable evidence
**When**: the archive gate evaluates the task
**Then**: the archive gate continues to emit the appropriate strict validation finding
