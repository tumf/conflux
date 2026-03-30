## MODIFIED Requirements

### Requirement: proposal-session-ui-chat

The Dashboard shall provide a chat interface for conversing with the ACP agent during a proposal session. Messages SHALL render Markdown including headings (h1-h3), unordered/ordered lists, links, horizontal rules, code blocks (with language labels and copy buttons), bold text, and inline code. Assistant messages SHALL show a copy button on hover. All messages SHALL display a relative timestamp on hover.

#### Scenario: send-and-receive-messages

**Given**: An active proposal session chat view
**When**: The user types a message and submits
**Then**: The message is sent via WebSocket, and agent responses stream into the message list with Markdown rendering

#### Scenario: tool-call-display

**Given**: The agent executes a tool during a prompt turn
**When**: Tool call updates arrive via WebSocket
**Then**: The tool call is displayed inline with its title and status (pending → in_progress → completed)

#### Scenario: render-markdown-headings-and-lists

**Given**: An assistant message containing Markdown headings and lists
**When**: The message is displayed
**Then**: Headings are rendered as styled h1-h3 elements, and lists are rendered with proper bullets/numbers and indentation

#### Scenario: render-markdown-links

**Given**: An assistant message containing `[text](url)` links
**When**: The message is displayed
**Then**: Links are rendered as clickable `<a>` elements that open in a new tab

#### Scenario: code-block-copy-button

**Given**: An assistant message containing a fenced code block
**When**: The message is displayed
**Then**: The code block shows a language label (if specified) and a copy button that copies the code content to clipboard

#### Scenario: message-copy-button

**Given**: An assistant message is displayed
**When**: The user hovers over the message
**Then**: A copy button appears that copies the full message content to clipboard when clicked

#### Scenario: message-timestamp-on-hover

**Given**: Any message is displayed
**When**: The user hovers over the message
**Then**: A relative timestamp (e.g., "2 min ago") is shown
