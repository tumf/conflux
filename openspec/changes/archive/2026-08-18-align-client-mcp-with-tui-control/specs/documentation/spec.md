## ADDED Requirements

### Requirement: Client documentation teaches explicit TUI-equivalent control and subscription

README, AGENTS guidance, bundled skills, CLI help, MCP descriptions, and integration documentation MUST describe the client as explicit TUI-equivalent control plus explicit proposal notification subscription. Documentation MUST NOT recommend `cflx_enqueue`, admission-oriented mark calls, automatic callback registration, or automatic agent/session resume.

Documentation MUST distinguish:

- execution mark: operator selection only;
- Start: explicit F5-equivalent lifecycle control over authoritative marks;
- queue intent: owner-side admission state produced by shared orchestration;
- proposal subscription: explicit notification registration that does not control workflow or resume an agent.

#### Scenario: MCP documentation lists the compact surface

- **WHEN** a reader inspects MCP documentation or tool descriptions
- **THEN** it lists `cflx_status`, `cflx_control`, and `cflx_subscribe`
- **AND** it explains the control and subscription actions
- **AND** it does not list historical enqueue or notify tools

#### Scenario: Mark documentation does not claim admission

- **WHEN** a reader inspects mark/unmark examples
- **THEN** the examples say mark writes preserve unrelated marks and return without admission
- **AND** they say owner-side settlement and analysis may later admit stable marked work
- **AND** they do not imply queue intent is part of the mark result

#### Scenario: Start and stop documentation mirrors TUI controls

- **WHEN** a reader inspects lifecycle-control examples
- **THEN** Start is described as F5/`!` equivalent
- **AND** Stop and ForceStop are described through their shared TUI semantics
- **AND** no client-specific lifecycle policy is documented

#### Scenario: Subscription documentation is explicit and proposal scoped

- **WHEN** a reader inspects notification guidance
- **THEN** it shows an explicit `cflx_subscribe` call over one or more proposal IDs
- **AND** it explains set/get/clear, future execution binding, owner-restart invalidation, and typed event fields
- **AND** it states that callback delivery does not automatically resume an agent or session

#### Scenario: Retired auto-resume guidance is absent

- **WHEN** repository documentation and bundled assets are searched
- **THEN** no active guidance recommends an OpenCode or Hermes auto-resume plugin, post-tool hook, or enqueue-triggered callback registration
- **AND** historical archived OpenSpec records may retain their immutable history
