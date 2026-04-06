## MODIFIED Requirements

### Requirement: post-archive-merge-dispatch

When a change is archived in parallel mode, the project-scoped reducer and orchestrator MUST classify the archived change according to whether another change in the same Project is already resolving.

If another change in the same Project has `ActivityState::Resolving`, the archived change MUST enter `ResolveWait` so it remains eligible for automatic continuation after the active resolve completes. Otherwise, the archived change MAY enter `MergeWait` for manual or immediate merge handling according to the existing post-archive flow.

#### Scenario: archive-completes-while-project-resolve-active

- **Given** Change A in a Project is in `Resolving`
- **And** Change B in the same Project has just been archived in parallel mode
- **When** the reducer processes `ChangeArchived` for Change B
- **Then** Change B enters `ResolveWait`
- **And** the derived display status is `resolve pending`

#### Scenario: archive-completes-without-project-resolve-active

- **Given** no other Change in the same Project is in `Resolving`
- **And** Change B has just been archived in parallel mode
- **When** the reducer processes `ChangeArchived` for Change B
- **Then** Change B enters `MergeWait`
- **And** the derived display status is `merge wait`
