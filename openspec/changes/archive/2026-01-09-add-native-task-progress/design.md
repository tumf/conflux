# Design: Native Task Progress Parsing

## Context

OpenSpec CLI's `list --json` command returns task counts from `tasks.md` files. However, the task detection regex in OpenSpec only matches bullet-style checkboxes:

```typescript
// OpenSpec's current regex (from task-progress.ts)
const TASK_PATTERN = /^[-*]\s+\[[\sx]\]/i;
```

This fails for numbered lists like `1. [x] Task`, causing `completedTasks: 0, totalTasks: 0` in the JSON output.

## Goals

- Parse tasks.md files natively in Rust
- Support both bullet (`- [ ]`) and numbered (`1. [ ]`) task formats
- Maintain backward compatibility with openspec CLI
- Provide accurate task progress even when openspec has parsing bugs

## Non-Goals

- Replacing openspec CLI entirely
- Parsing other openspec file formats (proposal.md, spec.md)

## Decisions

### Decision 1: Hybrid Approach

**What**: Use native parsing as primary, fall back to openspec CLI output when tasks.md doesn't exist.

**Why**: This provides immediate benefits while maintaining compatibility with projects that don't follow the standard structure.

**Alternatives considered**:
- CLI-only: Blocked by upstream bug
- Native-only: Would break if tasks.md location changes

### Decision 2: Regex Pattern for Task Detection

**Pattern**:
```rust
// Matches both bullet and numbered lists with checkboxes
r"^(?:[-*]|\d+\.)\s+\[([ xX])\]"
```

**Matches**:
- `- [ ] Task` (bullet unchecked)
- `- [x] Task` (bullet checked)
- `* [X] Task` (asterisk checked)
- `1. [ ] Task` (numbered unchecked)
- `10. [x] Task` (numbered checked)

**Doesn't match** (correctly):
- `  - Sub-item` (indented sub-bullets under checkboxes)
- `Some text [ ]` (inline checkboxes)
- `## [x] Header` (markdown headers)

### Decision 3: File Location

Tasks file location: `openspec/changes/{change_id}/tasks.md`

This matches OpenSpec's standard structure.

## Architecture

```
src/
├── task_parser.rs      # New module for native parsing
├── openspec.rs         # Updated to use native parser
└── ...
```

### TaskParser Module

```rust
pub struct TaskProgress {
    pub completed: u32,
    pub total: u32,
}

impl TaskParser {
    pub fn parse_file(path: &Path) -> Result<TaskProgress>;
    pub fn parse_content(content: &str) -> TaskProgress;
}
```

### Integration with list_changes

```rust
pub async fn list_changes(openspec_cmd: &str) -> Result<Vec<Change>> {
    let cli_result = fetch_from_cli(openspec_cmd).await?;

    for change in &mut cli_result {
        // If CLI reports 0/0, try native parsing
        if change.total_tasks == 0 {
            if let Ok(progress) = TaskParser::parse_change(&change.id) {
                change.completed_tasks = progress.completed;
                change.total_tasks = progress.total;
            }
        }
    }

    Ok(cli_result)
}
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| File path assumptions | Use configurable openspec directory path |
| Regex edge cases | Comprehensive unit tests with real-world examples |
| Performance overhead | Lazy parsing only when CLI returns 0/0 |

## Migration Plan

1. Add TaskParser module with comprehensive tests
2. Integrate with list_changes (non-breaking)
3. Add configuration option to prefer native parsing
4. Document the behavior difference

## Open Questions

- Should we emit a warning log when native parsing differs from CLI output?
- Should there be a CLI flag to force native parsing for debugging?
