# Design: pre-archive spec delta target validation

## Current behavior

Archive promotion merges deltas by canonical `### Requirement:` identity and rejects missing `MODIFIED` or `REMOVED` targets. Strict proposal validation currently checks structural shape, such as delta markers and scenarios, but does not perform the same canonical target existence check.

That means a proposal can pass strict validation, complete implementation, and only fail at archive when promotion discovers that the canonical target heading does not exist.

## Target behavior

The validator should perform a read-only version of archive promotion's target lookup during strict validation. It should not rewrite canonical specs or require archive execution. It should only parse canonical requirement headings and change-local delta headings, then report deterministic authoring errors.

## Matching semantics

The validation path should reuse the same normalized requirement identity rules used by archive promotion. This avoids split-brain behavior where validation passes but archive fails for the same target, or validation fails on a target archive would accept.

## Error phase

Missing `MODIFIED` and `REMOVED` targets are proposal authoring errors, not runtime archive blockers. They should therefore fail:

- `cflx openspec validate <id> --strict`
- `cflx openspec validate <id> --archive-gate`

The actual archive command should not be the first place the user sees this class of failure.

## Skill guidance

The bundled `cflx-proposal` skill should make the authoring contract explicit:

- inspect `openspec/specs/<capability>/spec.md`
- copy the exact canonical requirement heading when using `MODIFIED` or `REMOVED`
- use `ADDED` for new requirement identities
- validate with `cflx openspec validate <id> --strict` before handing off

## Constitution compliance

This design uses only repository files in the workspace as authoritative inputs. It does not add external logs, caches, or durable workflow-control state.
