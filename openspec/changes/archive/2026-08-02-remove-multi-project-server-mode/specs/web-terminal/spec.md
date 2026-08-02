## REMOVED Requirements

### Requirement: Terminal Session Metadata

Server-backed dashboard terminal sessions are removed.

#### Scenario: No server terminal metadata API

**Given**: The retained local web router
**When**: Its routes are enumerated
**Then**: It exposes no multi-project dashboard terminal-session resource

### Requirement: Terminal Session Scrollback Buffer

Persistent server terminal scrollback is removed.

#### Scenario: No server scrollback storage

**Given**: The production modules
**When**: terminal persistence is inspected
**Then**: No server dashboard scrollback store exists

### Requirement: Terminal Session Restoration on Page Reload

Server dashboard terminal restoration is removed.

#### Scenario: No terminal restoration contract

**Given**: The retained web product
**When**: a client reloads
**Then**: No multi-project server terminal restoration is promised

### Requirement: Terminal Tab Filtering by Worktree Context

Server dashboard terminal tabs and worktree filtering are removed.

#### Scenario: No server terminal tabs

**Given**: Packaged user interfaces
**When**: terminal UI surfaces are inspected
**Then**: No multi-project server terminal tab interface is present
