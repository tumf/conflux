## 1. Add Dependency

- [x] 1.1 Add `tui-textarea` crate to `Cargo.toml`

## 2. Add Configuration

- [x] 2.1 Add `propose_command: Option<String>` to `OrchestratorConfig`
- [x] 2.2 Implement `get_propose_command()` method (returns `None` when not configured)
- [x] 2.3 Add `expand_proposal()` function to `config/expand.rs` (`{proposal}` placeholder expansion)
- [x] 2.4 Add `propose_command` to configuration templates with comments

## 3. Add TUI Mode

- [x] 3.1 Add `AppMode::Proposing` to `tui/types.rs`
- [x] 3.2 Add `propose_textarea: Option<TextArea<'static>>` field to `AppState`
- [x] 3.3 Implement `AppState::start_proposing()` method (creates TextArea and switches mode)
- [x] 3.4 Implement `AppState::cancel_proposing()` method (clears TextArea and restores mode)
- [x] 3.5 Implement `AppState::submit_proposal()` method (gets text and restores mode)

## 4. Text Input Rendering

- [x] 4.1 Implement `render_propose_modal()` function in `tui/render.rs`
- [x] 4.2 Render modal dialog border and title
- [x] 4.3 Render text content inside modal
- [x] 4.4 Implement key hints in status bar for Proposing mode

## 5. Key Event Handling

- [x] 5.1 Add `+` key handling in `runner.rs` (triggers proposing mode)
- [x] 5.2 Implement key event dispatch for Proposing mode
- [x] 5.3 Handle character input, backspace, delete, enter, navigation keys
- [x] 5.4 Implement Ctrl+S for submit, Esc for cancel
- [x] 5.5 Implement warning message when propose_command is not configured

## 6. Command Execution

- [x] 6.1 Add `TuiCommand::SubmitProposal(String)` to `tui/events.rs`
- [x] 6.2 Implement command expansion logic (`{proposal}` → input text)
- [x] 6.3 Implement TUI suspension and command execution in `runner.rs`
- [x] 6.4 Implement success/error logging after command execution

## 7. Tests

- [x] 7.1 Unit tests for config parsing and expansion (`{proposal}` placeholder)
- [x] 7.2 Unit tests for mode transitions (Select → Proposing → Select)
- [x] 7.3 Unit tests for empty text submission handling

## 8. Documentation

- [x] 8.1 Add proposal input feature to README (`+` key, configuration example)
- [x] 8.2 Add `propose_command` to configuration file samples
