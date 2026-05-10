# Design: Repository-rooted dependency evidence for analyzer validation

## Context

Dependency target classification already models repository-visible states: queued, in-flight, archived, rejected, and missing. The bug is not the classification model; it is that analyzer-side archived/rejected evidence is collected from the process cwd instead of the repository root being orchestrated.

## Approach

Add repository-root ownership to analyzer validation:

1. `ParallelizationAnalyzer` should store a `repo_root: PathBuf` or receive one at validation time.
2. `collect_archived_change_ids` and `collect_rejected_change_ids` should be called with that root.
3. Existing callers should pass the same root used for OpenSpec listing and workspace scheduling.
4. Tests should create separate temp directories for target repo and process cwd to prove classification is root-stable.

## Constraints

- Do not use logs, runtime history, or UI state as dependency evidence.
- Do not relax missing/rejected dependency failure semantics.
- Do not make archived dependencies dispatch blockers; archived means already satisfied.

## Verification Strategy

Use temp repositories with minimal `openspec/changes` layouts to verify analyzer validation directly. The most important regression is that changing process cwd does not change the classification outcome for the same target repo contents.
