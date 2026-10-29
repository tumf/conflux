## Implementation Tasks

- [ ] 1.1 Settle accepted graceful stop at the positively proven inactive blocked-only scheduler boundary while preserving existing cleanup and waiter fences (verification: integration - persistent scheduler tests; verification-id: blocked-idle-graceful-stop)
- [ ] 1.2 Add deterministic positive, active-work, contention, cancellation and exactly-once controls through the real loop and shared lifecycle projections (verification: integration - API/TUI/wait projection assertions; verification-id: blocked-idle-graceful-stop)
