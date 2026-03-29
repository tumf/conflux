## Implementation Tasks

- [x] Add helper textarea lookup in `dashboard/src/components/TerminalTab.tsx` and keep it scoped to the opened xterm instance (verification: code review shows textarea access is local to terminal initialization)
- [x] Clear stale helper textarea content asynchronously after each forwarded `term.onData` input without changing PTY payload forwarding semantics (verification: `dashboard/src/components/TerminalTab.tsx` still sends `data` unchanged to WebSocket before textarea reset)
- [x] Preserve existing WebSocket output rendering and terminal resize behavior while applying the textarea workaround (verification: no changes outside client-side input path in `dashboard/src/components/TerminalTab.tsx`)
- [x] Keep or update reproducible debug instrumentation so the textarea accumulation can be observed before/after the fix (verification: `dashboard/public/debug-ws.js` or equivalent can show textarea value no longer grows after Ctrl+A + printable key)
- [x] Run `cd dashboard && npm run build` to verify the dashboard still compiles after the workaround

## Future Work

- Validate the workaround against a future xterm.js release and remove it if upstream behavior is corrected
- Add an automated browser regression test if the project later adopts stable terminal UI e2e coverage
