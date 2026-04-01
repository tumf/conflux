## Implementation Tasks

- [x] Add a dedicated proposal-chat system prompt constant/resource in the server-side proposal-session path, authored with `.opencode/agent/spec.md` as reference only and without runtime file reads (verification: source inspection in `src/server/proposal_session.rs` and/or adjacent prompt module shows the prompt is defined in Conflux code/resources, not loaded from `.opencode/agent/spec.md`).
- [x] Inject the dedicated prompt into ACP-backed proposal chat initialization so new sessions behave as a specification-focused assistant without ACP-native agent selection (verification: targeted server test covers session initialization / first-turn behavior through the ACP proposal-session path).
- [x] Update proposal-session backend behavior/tests to confirm runtime no longer relies on `OPENCODE_CONFIG`/mode selection for spec-oriented chat behavior (verification: `cargo test --test e2e_proposal_session` or equivalent proposal-session test target passes with assertions focused on prompt injection semantics).
- [x] Update OpenSpec proposal-session backend delta to describe dedicated prompt injection and specification-only behavior boundaries, while keeping `.opencode/agent/spec.md` out of the spec requirements themselves (verification: `python3 "/Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py" validate add-proposal-chat-system-prompt --strict`).
- [x] Run full verification for the implementation path (`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`) after code changes land (verification: all commands exit 0).

## Acceptance #1 Failure Follow-up

- [x] Update canonical proposal-session backend specs to remove the stale `OPENCODE_CONFIG` auto-injection / spec-agent requirements that contradict backend-managed prompt injection (`openspec/specs/proposal-session-backend/spec.md`).

## Acceptance #3 Failure Follow-up

- [x] Strengthen canonical `proposal-session-create` in `openspec/specs/proposal-session-backend/spec.md` so it explicitly requires backend-managed specification-focused prompt guidance during session initialization, matching the approved delta.
- [x] Strengthen canonical `proposal-session-websocket` in `openspec/specs/proposal-session-backend/spec.md` so prompt forwarding explicitly uses the session's backend-managed specification-focused guidance, matching the approved delta.
- [x] Deduplicate `openspec/specs/proposal-session-backend/spec.md` so the canonical file has a single authoritative `## Requirements` structure without repeated requirement blocks.

## Future Work

- Manually dogfood proposal chat in server-mode WebUI against a real `opencode acp` binary to confirm the conversation quality matches intended spec-oriented behavior.
