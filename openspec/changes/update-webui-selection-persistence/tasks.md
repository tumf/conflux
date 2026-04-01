## Implementation Tasks

- [ ] Persist selected browse target from `dashboard/src/App.tsx` whenever the user selects a change or worktree, using server-backed `ui_state` keys that can encode the current `fileBrowseContext` (verification: App-level or store-level frontend tests assert the expected UI-state API calls for change and worktree selection).
- [ ] Persist auxiliary tab state needed to surface restored selection (`desktopCenterTab`, `desktopRightTab`, and mobile `activeTab` where applicable) without regressing existing project/proposal-session restore behavior (verification: frontend tests assert restored selection drives the expected visible pane/tab combination after reload).
- [ ] Restore persisted browse target on initial FullState hydration only after validating the referenced project and currently available worktree/change context, and clear stale state keys when references are invalid (verification: reload-focused frontend tests cover valid change restore, valid worktree restore, and stale-reference cleanup).
- [ ] Preserve best-effort startup semantics so missing or deleted references do not block dashboard initialization or other restored state such as project/proposal session selection (verification: regression tests simulate stale persisted state and confirm normal dashboard rendering continues).
- [ ] Run verification for the dashboard persistence path (`npm test` for relevant frontend tests or equivalent targeted dashboard test command, plus the repo-standard lint/typecheck commands once implementation lands) (verification: documented command set exits 0 after implementation).

## Future Work

- Consider persisting deeper file-viewer state such as expanded directories or last-opened file if users want full workspace restoration beyond target selection.
