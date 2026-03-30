## ADDED Requirements

### Requirement: proposal-session-ui-mobile-changes-drawer

The Dashboard SHALL provide access to the proposal session Changes list on mobile viewports (below the `md` breakpoint) via a slide-in drawer accessible from the chat header.

#### Scenario: open-changes-drawer-on-mobile

**Given**: A proposal session chat view on a viewport narrower than 768px
**When**: The user taps the Changes toggle button in the chat header
**Then**: A drawer slides in from the right showing the ProposalChangesList with a backdrop overlay

#### Scenario: close-drawer-on-backdrop-tap

**Given**: The Changes drawer is open on mobile
**When**: The user taps the backdrop area outside the drawer
**Then**: The drawer closes

#### Scenario: close-drawer-on-escape

**Given**: The Changes drawer is open on mobile
**When**: The user presses the Escape key
**Then**: The drawer closes

#### Scenario: drawer-hidden-on-desktop

**Given**: A proposal session chat view on a viewport 768px or wider
**When**: The chat view is displayed
**Then**: The Changes toggle button is not visible and the sidebar renders inline as before
