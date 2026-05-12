## MODIFIED Requirements

### Requirement: Native validator owns behavior-centric proposal checks

The native `cflx openspec validate` implementation MUST enforce deterministic proposal authoring contracts such as structural validity, verification-note presence, supported evidence enum usage, spec delta target existence, and other repository-verifiable formatting rules. It MUST NOT infer implementation-task adequacy solely from wording heuristics about runtime behavior claims or whether tasks appear implementation-facing.

For strict validation, every `MODIFIED Requirements` and `REMOVED Requirements` block MUST target a requirement identity that exists in the corresponding canonical `openspec/specs/<capability>/spec.md` file. Missing targets MUST fail validation before archive promotion.

#### Scenario: validator rejects missing modified target before archive

**Given**: a change delta under `openspec/changes/alpha/specs/demo/spec.md` contains `## MODIFIED Requirements`
**And**: it includes `### Requirement: Missing Requirement`
**And**: canonical `openspec/specs/demo/spec.md` does not contain that requirement identity
**When**: `cflx openspec validate alpha --strict` is executed
**Then**: validation fails
**And**: the diagnostic says `MODIFIED target not found in canonical spec`
**And**: archive promotion is not required to discover the missing target

#### Scenario: validator rejects missing removed target before archive

**Given**: a change delta under `openspec/changes/alpha/specs/demo/spec.md` contains `## REMOVED Requirements`
**And**: it includes `### Requirement: Missing Requirement`
**And**: canonical `openspec/specs/demo/spec.md` does not contain that requirement identity
**When**: `cflx openspec validate alpha --strict` is executed
**Then**: validation fails
**And**: the diagnostic says `REMOVED target not found in canonical spec`
**And**: archive promotion is not required to discover the missing target

#### Scenario: added requirements do not require existing canonical targets

**Given**: a change delta under `openspec/changes/alpha/specs/demo/spec.md` contains `## ADDED Requirements`
**And**: it includes `### Requirement: New Requirement`
**And**: canonical `openspec/specs/demo/spec.md` does not contain that requirement identity
**When**: `cflx openspec validate alpha --strict` is executed
**Then**: validation does not fail because the added requirement lacks a canonical target

#### Scenario: archive gate reports the same missing target blocker

**Given**: a change delta contains a missing `MODIFIED` or `REMOVED` target
**When**: `cflx openspec validate alpha --archive-gate` is executed
**Then**: validation fails with the missing-target diagnostic before `cflx openspec archive alpha --yes` is needed
