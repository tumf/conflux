## REMOVED Requirements

### Requirement: proposal-session-opencode-config

Removed with server proposal sessions.

#### Scenario: No proposal-session OpenCode config

**Given**: Current configuration
**When**: supported fields are inspected
**Then**: No server proposal-session OpenCode config exists

### Requirement: auto-generate-opencode-proposal-config

Removed with server proposal sessions.

#### Scenario: No generated proposal config

**Given**: A local workspace
**When**: configuration is generated
**Then**: No server proposal-session config is generated

### Requirement: Proposal Session Database-Backed Lifecycle

Removed with server proposal sessions.

#### Scenario: No database-backed proposal lifecycle

**Given**: Production modules
**When**: proposal lifecycles are enumerated
**Then**: No server database-backed lifecycle exists

### Requirement: Proposal Session Message Database Persistence

Removed with server proposal sessions.

#### Scenario: No proposal message persistence

**Given**: Production persistence
**When**: schemas are inspected
**Then**: No server proposal message table exists

### Requirement: Proposal planning assigns verification ownership

Removed from the server proposal-session capability; current proposal authoring remains governed by active workflow specs.

#### Scenario: No server-session planning contract

**Given**: The retained product
**When**: proposal entrypoints are enumerated
**Then**: No server proposal-session planner exists

### Requirement: Proposal skill standardizes verification coverage vocabulary

Removed from the obsolete server proposal-session capability.

#### Scenario: No server-session skill contract

**Given**: The retained product
**When**: server proposal-session surfaces are inspected
**Then**: No server-session skill surface exists

### Requirement: Proposal tasks reflect verification coverage plan

Removed from the obsolete server proposal-session capability.

#### Scenario: No server-session task contract

**Given**: The retained product
**When**: proposal-session tasks are inspected
**Then**: No server-session task flow exists

### Requirement: Proposal workflow guidance must align with explicit structural validation

Removed from the obsolete server proposal-session capability.

#### Scenario: No server-session workflow guidance

**Given**: The retained product
**When**: proposal-session workflows are enumerated
**Then**: No server-session workflow exists
