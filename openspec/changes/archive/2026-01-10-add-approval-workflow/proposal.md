# Proposal: Add Approval Workflow

## Summary

Add an approval mechanism allowing users to explicitly approve changes before they can be automatically queued for processing. This prevents accidental execution of unapproved specifications and provides a clear audit trail of user consent.

## Problem

Currently, any change in `openspec/changes/` can be queued and executed without explicit user approval. This creates risk of:
- Executing incomplete or draft specifications
- No clear record of user consent for specifications
- Difficulty tracking which version of a specification was approved

## Solution

Introduce an `approved` file mechanism:
- Create `openspec/changes/{change_id}/approved` to mark a change as approved
- The file contains MD5 checksums of all specification files at approval time
- Approval status is validated by comparing current file hashes against the approved manifest
- Only approved changes can be queued for automatic execution

## Key Features

1. **Approval File Structure**: MD5 checksums of all `.md` files in the change directory
2. **Validation Logic**: Compare hashes excluding `tasks.md` (which changes during execution)
3. **CLI Commands**: `approve set/unset {change_id}` for managing approval status
4. **TUI Integration**: `@` key toggles approval, approved changes auto-queue on startup
5. **Run Behavior**: Unapproved changes cannot be added to queue, warning shown if attempted

## Out of Scope

- Multi-user approval workflows
- Approval expiration
- Cryptographic signatures (MD5 is sufficient for integrity checking)

## Impact

- CLI: New `approve` subcommand
- TUI: New `@` key binding, visual indicator for approval status
- Orchestrator: Queue filtering based on approval status
- Configuration: No changes required
