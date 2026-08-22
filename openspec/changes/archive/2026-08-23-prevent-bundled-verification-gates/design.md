# Design

## Decision

Keep the existing verification model. Add a validator check over parseable task verification references rather than a second task taxonomy or runtime task selector.

## Cohesion key

For every active checkbox reference, parse:

- `verification-id`
- standard verification ownership marker (`unit`, `integration`, `e2e`, `manual`, `benchmark`, `not-testable`)
- concrete command text
- source line

The ownership marker is the single case-insensitive token immediately after `verification:` and before the first ` - `. It MUST exactly match the closed set above; zero or multiple matches produce a diagnostic instead of a silent pass. `benchmark` remains a valid marker for non-blocking declarations, but benchmark command forms cannot be change-blocking.

Normalize a concrete command by removing Markdown backticks, folding internal whitespace to one ASCII space, trimming, and comparing case-insensitively. A reused change-blocking ID is cohesive only when the extracted ownership marker and normalized concrete command match. This permits one focused command to cover coupled code-and-test work. It does not prove semantic cohesion when unrelated tasks intentionally declare an identical pair.

## Heavy command boundary

Use a small deterministic syntax policy. It examines every declared command form (`evidence`, `rerun`, task-line concrete commands, and structured argv when present), never prose. Initial rejected change-blocking forms cover container orchestrators, architecture emulators, benchmark selectors, explicit full/exhaustive/heavy selectors, and repeated execution loops. The structural denylist is a native-validator rule and takes precedence over guidance that otherwise permits a bounded repository-local integration path and over generic evidence-hint recognition. A bounded local path must use a command form that does not match the denylist. A denylist diagnostic is distinct from `missing repository evidence` and prescribes a short repository-local smoke command plus a separate operational owner.

## Migration

The rule applies during proposal validation. Existing archived proposals remain historical evidence. Active proposals receive actionable errors only from explicit parseable syntax.
