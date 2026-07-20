# Design: Deterministic verification phases

## Context

Conflux can truthfully decide acceptance and archive readiness only from repository-verifiable evidence. Post-integration services may produce valuable operational evidence, but that evidence is unavailable before the change is merged or pushed and cannot be an authoritative pre-integration routing input under the Constitution.

## Decision

### Metadata shape

Proposal frontmatter may contain:

```yaml
verifications:
  - id: deployed-smoke
    requirement: Repository validates and deploys the site
    phase: post-integration
    owner: repository-automation
    trigger: default-branch-integration
    automation: .github/workflows/pages.yml
    evidence: github-actions:pages#verify-deployment
    rerun: rerun the failed Pages workflow job
    prerequisites:
      - GitHub Pages uses GitHub Actions as its source
```

Required entry fields:

- `id`: unique, non-empty stable identity within the change
- `requirement`: non-empty requirement or scenario identity covered by the verification
- `phase`: `pre-integration` or `post-integration`
- `owner`: `conflux-acceptance` or `repository-automation`
- `trigger`: non-empty execution trigger
- `automation`: tracked repository-relative regular file
- `evidence`: expected result or evidence location
- `rerun`: concrete recovery or rerun action
- `prerequisites`: list of external prerequisites; empty list is valid

`pre-integration` requires `owner: conflux-acceptance`. `post-integration` requires `owner: repository-automation`.

### Validation boundary

The native validator checks syntax, required values, enum relationships, duplicate IDs, and path safety. It resolves `automation` without allowing absolute paths, `..`, or symlinks escaping the repository. It does not parse provider-specific workflow semantics and does not access networks.

Implementation and hybrid proposals must declare at least one pre-integration verification. Spec-only proposals may omit declarations. Existing metadata consumers retain tolerant reads; strict validation uses a diagnostic-preserving parser so malformed metadata cannot silently become absent metadata.

Natural-language hints may warn that a condition appears misclassified, but they never create, replace, or route a verification declaration.

### Acceptance boundary

For pre-integration entries, acceptance evaluates current-revision repository evidence and runnable local verification.

For post-integration entries, acceptance verifies that the tracked automation, trigger, evidence publication, rerun action, and prerequisites are coherently implemented. It does not require evidence that can only exist after integration, and it does not fetch the external target.

Missing or placeholder repository automation is a FAIL. A non-mockable external prerequisite that prevents even the declared automation from being viable is a stalled hold with a preserved next action. A correctly wired automation whose operational run is merely pending does not block pre-integration acceptance.

### Persistence

The declaration remains in `proposal.md`; archive already moves the entire change directory. No new state store is introduced. External workflow results remain observability and operational evidence, not Conflux routing state.

## Alternatives Rejected

- Natural-language-only phase inference: nondeterministic and multilingual.
- Moving all deployment checks to unstructured Future Work: execution ownership and evidence are lost.
- Adding a Conflux remote deployment lifecycle now: unnecessary scope and constitutional conflict.
- Provider-specific metadata: couples OpenSpec authoring to one CI/CD system.

## Migration

Existing spec-only proposals remain valid. Active implementation/hybrid proposals must add a pre-integration declaration before strict validation succeeds. Existing post-integration completion language must be reclassified with an explicit automation owner rather than deleted or weakened.
