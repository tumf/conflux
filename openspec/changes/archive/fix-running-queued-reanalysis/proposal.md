# Change: Fix queued reanalysis after slot release in running mode

## Problem / Context

In parallel TUI execution, a user can start a manual resolve from a `MergeWait` change and then queue additional changes while the TUI remains in `Running` mode. In the current behavior, those additional queued changes can remain idle even after an execution slot becomes available.

This happens because dynamic queue additions are gated by debounce behavior, and manual resolve completion is not treated as a scheduler completion trigger. As a result, the system can appear stuck with queued changes visible in the UI even though dispatch capacity has returned.

## Proposed Solution

- Treat execution slot release as a first-class trigger for parallel re-analysis when queued changes exist.
- Ensure manual resolve completion causes the scheduler to re-evaluate queued changes using the same slot accounting rules as other active work.
- Preserve debounce for bursty queue edits, but do not let debounce delay execution once capacity becomes available.
- Clarify TUI-visible waiting semantics so queued changes are not perceived as silently stalled.

## Acceptance Criteria

1. When queued changes exist during `Running` mode and available slots transition from zero to positive, re-analysis starts without waiting for the normal queue debounce window.
2. Completion of a manual resolve that had consumed a parallel slot triggers re-evaluation of queued changes.
3. Queued changes are dispatched as soon as dependency order and slot availability allow, without requiring a separate task completion event from apply/archive work.
4. The UI state model continues to preserve explicit queued intent and does not regress queued rows to `not queued` before analysis/dispatch begins.
5. Existing debounce behavior for repeated queue edits remains allowed when no slot-availability transition has occurred.

## Out of Scope

- Changing dependency analysis criteria or prompt content.
- Redesigning the overall resolve workflow beyond slot-triggered re-analysis.
- Introducing new remote/server-side scheduling semantics.
