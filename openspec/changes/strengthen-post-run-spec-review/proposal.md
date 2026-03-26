# Change: Strengthen post-run spec review

## Why
`cflx run` currently emphasizes base-branch cleanliness, recent commits, and a top-level `git diff`, but it does not require a semantic review of the canonical specs that changed during archive. Session analysis showed that this leaves a blind spot: a spec-only change can appear to have completed successfully even when the canonical `openspec/specs/**` diff is empty or incorrect.

The post-run checklist should make canonical spec review an explicit part of the operator workflow so archive promotion problems are visible immediately after orchestration.

## What Changes
- Expand the `cflx-run` review checklist to require direct review of canonical `openspec/specs/**` diffs after orchestration.
- Require a per-change summary of which canonical specs changed for each archived change that landed.
- Flag spec-only changes that land without a canonical spec diff as anomalous and worthy of investigation.
- Document how the post-run review should combine commit inspection with spec inspection.

## Impact
- Affected specs: `post-run-review`
- Affected code: `skills/cflx-run/SKILL.md`
- Dependencies: complements `update-spec-archive-promotion`; it can ship independently, but its anomaly checks become more actionable once archive-check semantics exist

## Non-Goals
- Changing Conflux runtime orchestration behavior or merge mechanics
- Replacing the existing git-status or commit-log review steps
- Reclassifying proposal types inside the run skill itself

## Success Criteria
- The documented `cflx run` review path includes a canonical spec diff check.
- Operators are told to summarize canonical spec changes by archived change, not only by commit.
- The review checklist explicitly flags spec-only changes whose canonical spec diff is empty.
