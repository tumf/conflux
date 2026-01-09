# Add Version Display

## Summary

Add version information display to CLI `--version` option and TUI footer.

## Motivation

- Users need to verify which version of the tool they are running
- Version information helps with debugging and support requests
- Standard CLI practice to support `--version` flag
- TUI footer has space to display version info

## Scope

- **CLI**: Add `--version` / `-V` flag support via clap
- **TUI**: Display version in selection mode footer

## Approach

1. Use clap's built-in `#[command(version)]` attribute to enable `--version`
2. Add version display to TUI footer (right-aligned)

## Related Specs

- `openspec/specs/cli/spec.md` - CLI specification (will add version requirement)
