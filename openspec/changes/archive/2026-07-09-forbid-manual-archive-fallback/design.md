# Design: Archive layout validation and no manual fallback

## Context

Conflux already defines the native archive writer as `cflx openspec archive <change_id> --yes`, which writes `openspec/changes/archive/YYYY-MM-DD-<change_id>/`.
Read paths keep compatibility with older direct archives at `openspec/changes/archive/<change_id>/`.
The incident path `openspec/changes/archive/YYYY-MM-DD/<change_id>/` is neither format.

## Decisions

### CLI owns archive mutation

The archive agent must not implement its own archive writer.
The only supported mutation surface is the native CLI archive command because it owns validation, canonical spec promotion, destination naming, and destination collision behavior.

### Invalid layout is fail-closed

Any archive state for a requested `change_id` under a nested date directory must be treated as invalid archive layout.
The diagnostic should name both the bad path and expected `openspec/changes/archive/YYYY-MM-DD-<change_id>` shape.

### Read compatibility is preserved

Existing direct archive entries remain readable for compatibility, but new archive generation remains date-prefixed.
This change validates shape; it does not migrate or delete existing valid archives.

## Implementation Notes

A small shared helper is preferred over duplicating path rules.
It should be able to answer:

- valid archive entry path for a `change_id`, if one exists
- invalid matching nested archive path, if one exists
- whether archive completion can be considered valid

Callers that currently check only `name == change_id || name.ends_with("-<change_id>")` should use the helper or mirror its exact validation.

## Failure Message Shape

Example diagnostic:

```text
Invalid archive layout for '<change_id>': found nested archive path openspec/changes/archive/2026-07-09/<change_id>. Expected openspec/changes/archive/YYYY-MM-DD-<change_id>. Do not manually move archive directories; restore the active change and rerun cflx openspec archive <change_id> --yes.
```

## Alternatives Considered

### Auto-repair nested archive layout

Rejected.
Automatic repair would make another component an archive writer and risks hiding the same class of bookkeeping failure.

### Accept nested date directories as another legacy format

Rejected.
The native CLI never writes that format, and accepting it would codify a manual fallback bug.
