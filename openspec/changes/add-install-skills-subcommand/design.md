## Context

This change introduces a new CLI behavior, a new dependency (`agent-skills-rs`), and a bundled-skills packaging convention. The user explicitly wants the implementation model to match `agent-exec`, especially the meaning of `self` as bundled skills sourced from a top-level `skills/` directory.

## Goals / Non-Goals

- Goals:
  - Add a minimal, script-friendly `install-skills` subcommand.
  - Keep source handling intentionally narrow: `self` and `local:<path>` only.
  - Keep install destination and lock-file scope aligned.
  - Establish a top-level `skills/` layout as the bundled source of truth.
- Non-Goals:
  - Remote git or registry-backed skill sources.
  - Uninstall/update/list subcommands.
  - Agent-exec-style command introspection in this proposal.

## Decisions

- Decision: Reuse the existing `cli.rs` + `main.rs` command routing pattern for a new `InstallSkills` subcommand.
  - Why: This matches the repository's current CLI architecture and keeps the change discoverable.

- Decision: Define `self` as bundled skills from a top-level `skills/` directory.
  - Why: This matches the user's stated requirement and the referenced `agent-exec` convention.

- Decision: Support only `self` and `local:<path>` in the first version.
  - Why: This keeps the proposal scoped and avoids premature source-scheme complexity.

- Decision: Keep project/global install directories and lock files in the same scope.
  - Why: Mixed-scope installs and lock tracking would be surprising and harder to reason about.

## Risks / Trade-offs

- Bundled `skills/` content becomes part of Conflux's release surface.
  - Mitigation: Keep the initial bundled set small and document the layout clearly.

- Path handling for `local:<path>` and global installs can be platform-sensitive.
  - Mitigation: Add filesystem-focused tests that assert exact resolved destinations and lock paths.

- If the current in-repo `.agents/skills/` content remains the only authored skill location, maintainers could be unsure which tree is canonical.
  - Mitigation: Document that bundled `self` installs source from top-level `skills/`, while `.agents/skills/` remains an internal or local-authoring location unless explicitly migrated.

## Migration Plan

1. Add the new CLI surface and dependency.
2. Introduce top-level bundled `skills/` content for `self` installs.
3. Implement install logic and path resolution.
4. Add tests for parsing and filesystem behavior.
5. Update user-facing docs.

## Open Questions

- Whether the existing `.agents/skills/refactor` content should be copied or migrated into the new top-level `skills/` tree as part of this proposal.
