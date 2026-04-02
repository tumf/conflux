## Implementation Tasks

- [x] Update `dashboard/src/components/ChatInput.tsx` so a successful send clears the textarea immediately after submission handoff while preserving the existing textarea editability rules (verification: `dashboard/src/components/__tests__/ChatInput.test.tsx` asserts the textarea is emptied after send).
- [x] Align `dashboard/src/hooks/useProposalChat.ts` recovery handling with the proposal-session turn-state contract so reconnect completion transitions the session back to `ready` and re-enables sending (verification: `dashboard/src/hooks/useProposalChat.test.ts` covers reconnect recovery ending in `ready`).
- [x] Add or update frontend regression tests covering normal submit-clear behavior and the stuck-responding recovery path (verification: targeted Vitest cases for `ChatInput` and `useProposalChat` pass).
- [x] Run dashboard verification for this fix, including targeted frontend tests and the repo-standard lint/typecheck commands before implementation is considered complete (verification: `npm test -- ChatInput useProposalChat` or equivalent targeted dashboard test command, plus `cargo fmt --check` and `cargo clippy -- -D warnings` after implementation changes land).

## Future Work

- Consider adding a defensive timeout or telemetry for cases where the backend never emits `turn_complete`, if real-world reports continue after the frontend regression fix.
