# Proposal: Add Ctrl+C as TUI Quit Shortcut

## Summary

Add `Ctrl+C` as an alternative keyboard shortcut to quit the TUI dashboard, in addition to the existing `q` key.

## Why

- `Ctrl+C` is a universal terminal shortcut that users instinctively reach for to exit applications
- Improves user experience by supporting both vim-style (`q`) and standard terminal conventions (`Ctrl+C`)
- Reduces user frustration when `q` is not immediately obvious as the quit key

## Scope

- **Affected capability**: `cli` (TUI keyboard handling)
- **Files affected**: `src/tui.rs` (key event handling)
- **Risk level**: Low - additive change with no behavior modification

## Approach

1. Import `KeyModifiers` from crossterm
2. Add pattern matching for `Ctrl+C` alongside existing `q` key handler

## Related Specs

- `cli/spec.md` - Requirement: 変更選択モード / Scenario: 終了
