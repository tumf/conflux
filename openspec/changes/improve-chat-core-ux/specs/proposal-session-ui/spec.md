## ADDED Requirements

### Requirement: proposal-session-ui-smart-scroll

The Dashboard chat message list SHALL auto-scroll to the bottom only when the user is already near the bottom of the scroll area (within 100px). When the user has scrolled up and new content arrives, a "New messages" indicator button SHALL appear. Clicking the indicator SHALL scroll to the bottom.

#### Scenario: no-forced-scroll-when-reading-history

**Given**: The user has scrolled up more than 100px from the bottom in an active chat
**When**: A new streaming chunk or message arrives
**Then**: The scroll position remains unchanged and a "New messages" pill button appears at the bottom of the viewport

#### Scenario: click-new-messages-scrolls-to-bottom

**Given**: The "New messages" pill button is visible
**When**: The user clicks it
**Then**: The chat scrolls to the bottom and the pill disappears

### Requirement: proposal-session-ui-enter-to-send

The Dashboard chat input SHALL send the message when the user presses Enter (without modifier keys). Shift+Enter SHALL insert a newline.

#### Scenario: enter-sends-message

**Given**: The chat input has text and is enabled
**When**: The user presses Enter
**Then**: The message is sent and the input is cleared

#### Scenario: shift-enter-inserts-newline

**Given**: The chat input has text and is enabled
**When**: The user presses Shift+Enter
**Then**: A newline character is inserted at the cursor position without sending

### Requirement: proposal-session-ui-typing-indicator

The Dashboard SHALL display a typing indicator (animated dots) when the agent is responding but no streaming content has been received yet.

#### Scenario: show-typing-indicator-before-stream

**Given**: A prompt has been submitted and `isAgentResponding` is true
**When**: No streaming content or tool calls have been received for the current turn
**Then**: An animated typing indicator with "Agent is thinking..." text is displayed below the last message

#### Scenario: hide-typing-indicator-on-stream-start

**Given**: The typing indicator is visible
**When**: The first streaming chunk or tool call arrives
**Then**: The typing indicator is hidden and replaced by the streaming content

### Requirement: proposal-session-ui-empty-state

The Dashboard SHALL display an informative empty state when a proposal session has no messages, including an icon, description, and example prompts.

#### Scenario: new-session-empty-state

**Given**: A newly created proposal session with no messages
**When**: The chat view is displayed
**Then**: A Bot icon, descriptive text ("Start a conversation..."), and at least 2 clickable example prompts are shown
