## MODIFIED Requirements

### Requirement: proposal-session-ui-elicitation

The Dashboard shall render ACP form-mode elicitation requests as interactive UI forms. The elicitation dialog SHALL be an accessible modal dialog with `role="dialog"`, `aria-modal="true"`, focus trapping, and keyboard dismiss via Escape.

#### Scenario: render-enum-selection

**Given**: An elicitation request with a string property using `oneOf` enum values
**When**: The elicitation is displayed
**Then**: A select/radio input is rendered with the enum options and the user can choose one

#### Scenario: submit-elicitation-response

**Given**: An elicitation form is displayed
**When**: The user fills in the form and clicks submit
**Then**: An `accept` response with the form data is sent via WebSocket

#### Scenario: cancel-elicitation

**Given**: An elicitation form is displayed
**When**: The user dismisses the dialog
**Then**: A `cancel` response is sent via WebSocket

#### Scenario: escape-closes-elicitation

**Given**: An elicitation form is displayed
**When**: The user presses the Escape key
**Then**: A `cancel` response is sent via WebSocket and the dialog closes

#### Scenario: focus-trap-within-elicitation

**Given**: An elicitation form is displayed
**When**: The user presses Tab repeatedly
**Then**: Focus cycles within the dialog and does not escape to elements behind the overlay

## ADDED Requirements

### Requirement: proposal-session-ui-semantic-tokens

The Dashboard chat components SHALL use semantic color tokens defined in the CSS `@theme` block rather than hardcoded hex color values. This ensures consistency and enables future theming.

#### Scenario: no-hardcoded-hex-in-chat-components

**Given**: The chat-related component source files (ProposalChat, ChatMessageList, ChatInput, ToolCallIndicator, ProposalChangesList, ElicitationDialog)
**When**: The source code is inspected
**Then**: No hardcoded hex color values (e.g., `#27272a`, `#6366f1`) are used in Tailwind class names; all colors reference semantic tokens
