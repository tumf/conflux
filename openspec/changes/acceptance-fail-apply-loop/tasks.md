## 1. Spec Updates
- [ ] 1.1 Update CLI acceptance loop requirements to return to apply with task edits and continued iteration
- [ ] 1.2 Update parallel acceptance loop requirements to return to apply with task edits and continued iteration

## 2. Task Update Rules
- [ ] 2.1 Define how acceptance failure edits tasks.md (add follow-up task or uncheck a completed task)
- [ ] 2.2 Define how failure reasons are recorded alongside task edits

## 3. Implementation
- [ ] 3.1 Update serial acceptance handling to edit tasks.md and return to apply with the same iteration counter
- [ ] 3.2 Update parallel acceptance handling to edit tasks.md and return to apply with the same iteration counter
- [ ] 3.3 Ensure acceptance failure emits events that move status back to apply (not completed)

## 4. Validation
- [ ] 4.1 Run targeted tests for acceptance failure transitions
- [ ] 4.2 Run `cargo test` if acceptance or orchestrator tests exist
