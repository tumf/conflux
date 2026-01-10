# Design: Remove OpenSpec CLI Dependency

## Current Architecture

```
Orchestrator::run()
    └── openspec::list_changes(openspec_cmd)  // Async, external command
            └── Command::new("sh").arg("-c").arg("npx @fission-ai/openspec@latest list --json")
```

## Target Architecture

```
Orchestrator::run()
    └── openspec::list_changes_native()  // Sync, filesystem read
            └── fs::read_dir("openspec/changes")
            └── task_parser::parse_change(change_id)
```

## Key Changes

### 1. Orchestrator Struct Simplification

**Before:**
```rust
pub struct Orchestrator {
    agent: AgentRunner,
    openspec_cmd: String,  // Remove this
    // ...
}
```

**After:**
```rust
pub struct Orchestrator {
    agent: AgentRunner,
    // openspec_cmd removed
    // ...
}
```

### 2. Run Method Changes

**Before:**
```rust
let initial_changes = openspec::list_changes(&self.openspec_cmd).await?;
// ...
let changes = openspec::list_changes(&self.openspec_cmd).await?;
```

**After:**
```rust
let initial_changes = openspec::list_changes_native()?;
// ...
let changes = openspec::list_changes_native()?;
```

Note: `list_changes_native()` is synchronous, not async. The `?` propagation still works.

### 3. Constructor Signature

**Before:**
```rust
pub fn new(
    openspec_cmd: &str,
    target_change: Option<String>,
    config_path: Option<PathBuf>,
) -> Result<Self>
```

**After:**
```rust
pub fn new(
    target_change: Option<String>,
    config_path: Option<PathBuf>,
) -> Result<Self>
```

## Migration Notes

- The `list_changes_native()` function already includes the native task parsing fallback
- No async runtime changes needed since `list_changes_native()` is sync
- Test mocking may need adjustment since we can't mock the command anymore
