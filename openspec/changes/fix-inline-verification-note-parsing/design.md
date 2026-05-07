# Design: Inline verification note parsing

## Current behavior

`validate_tasks_content` in `src/openspec_cmd.rs` uses an inline verification regex anchored to the end of a checkbox task. This treats a note as missing when the task line contains valid inline verification followed by additional completion-condition prose.

## Target behavior

The validator should treat `(verification: ...)` as a structured inline annotation wherever it appears in the checkbox task text. Text after the closing parenthesis remains ordinary task prose and must not invalidate the annotation.

## Parser constraints

- Capture only the text inside the verification annotation.
- Continue to support standalone indented `verification:` continuation lines.
- Continue to require ownership markers (`unit`, `integration`, `e2e`, `manual`, `benchmark`, `not-testable`) when evidence checks are enabled.
- Continue to require repository-verifiable evidence hints when evidence mode is `warn` or `error`.
- Do not weaken self-referential final validation checkbox detection.

## Risk

The main risk is overmatching unrelated parenthetical text. Limiting the match to the explicit `verification:` prefix and the next closing parenthesis keeps the behavior deterministic and compatible with existing task authoring guidance.
