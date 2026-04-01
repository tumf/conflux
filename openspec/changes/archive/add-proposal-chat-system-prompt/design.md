## Context

Proposal chat in server mode now runs through ACP-backed proposal sessions. The intended UX is a specification-focused conversation, but ACP does not expose proposal-session agent selection. The backend therefore needs to supply the role guidance itself.

The prompt wording can be authored with `.opencode/agent/spec.md` as a reference, but that file is not part of the runtime contract and must not become a startup or behavioral dependency.

## Goals / Non-Goals

- Goals:
  - Make proposal chat behave like a specification-focused assistant.
  - Realize that behavior through backend-managed prompt injection.
  - Keep runtime independent from `.opencode/agent/spec.md` and ACP-native agent selection.
- Non-Goals:
  - Add new ACP protocol features.
  - Make proposal chat perform implementation work.
  - Introduce prompt synchronization machinery.

## Decisions

- Decision: Use a dedicated Conflux-managed prompt for proposal chat.
  - Why: ACP cannot select the desired agent mode for proposal sessions, so the backend must provide equivalent behavioral guidance itself.
- Decision: Treat `.opencode/agent/spec.md` as authoring reference only.
  - Why: It is useful prompt source material, but runtime behavior should stay self-contained and deterministic within Conflux.
- Decision: Specify behavioral outcomes in OpenSpec, not provenance of the prompt wording.
  - Why: Specs should describe required system behavior and boundaries, while proposal/design docs may record implementation references.

## Risks / Trade-offs

- Prompt drift from `.opencode/agent/spec.md`
  - Mitigation: accept drift as intentional; proposal/design docs can name the reference used when updating wording.
- ACP session initialization may not have an explicit system-prompt primitive
  - Mitigation: implement prompt injection at the earliest reliable backend-managed turn boundary and test observable behavior rather than internal transport assumptions.

## Migration Plan

1. Define the dedicated proposal-chat prompt in Conflux code/resources.
2. Inject it into new ACP-backed proposal chat sessions.
3. Replace outdated config/spec assumptions that tie spec-oriented behavior to OPENCODE_CONFIG or agent selection.
4. Validate with proposal-session tests and full Rust verification.

## Open Questions

- Which exact backend injection point is the most stable for ACP session initialization in the current implementation?
