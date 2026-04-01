## ADDED Requirements

### Requirement: proposal-session-messages-endpoint

The server SHALL provide a REST endpoint `GET /api/v1/projects/{id}/proposal-sessions/{session_id}/messages` that returns the persisted message history for a proposal session as a JSON array of `ProposalSessionMessageRecord` objects.

#### Scenario: get-messages-for-active-session

**Given**: An active proposal session with user and assistant messages
**When**: A GET request is made to `/api/v1/projects/{id}/proposal-sessions/{session_id}/messages`
**Then**: The response is 200 OK with a JSON array of message records in chronological order

#### Scenario: get-messages-for-nonexistent-session

**Given**: No proposal session with the given session ID exists
**When**: A GET request is made to `/api/v1/projects/{id}/proposal-sessions/{session_id}/messages`
**Then**: The response is 404 Not Found
