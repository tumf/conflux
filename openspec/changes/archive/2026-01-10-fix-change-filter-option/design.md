# Design: Fix --change Option Filtering

## Current Architecture

```
CLI (--change) → main.rs → Orchestrator::new(target_change: Option<String>)
                                ↓
                        run() → list_changes() → ALL changes in snapshot
                                ↓
                        filter by target_change (late filtering)
```

**Problem**: Snapshot is captured BEFORE filtering, so logs show all changes.

## Proposed Architecture

```
CLI (--change a,b,c) → parse as Vec<String> → main.rs → Orchestrator::new(target_changes: Option<Vec<String>>)
                                                            ↓
                                                    run() → list_changes()
                                                            ↓
                                                    filter by target_changes (early filtering)
                                                            ↓
                                                    warn about missing changes
                                                            ↓
                                                    snapshot only includes valid targets
```

## Code Changes

### 1. src/cli.rs

```rust
#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Process only the specified changes (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub change: Option<Vec<String>>,
    // ...
}
```

### 2. src/orchestrator.rs

```rust
pub struct Orchestrator {
    // ...
    target_changes: Option<Vec<String>>,  // Renamed from target_change
    // ...
}

impl Orchestrator {
    pub fn new(
        openspec_cmd: &str,
        target_changes: Option<Vec<String>>,  // Changed type
        config_path: Option<PathBuf>,
    ) -> Result<Self> {
        // ...
    }

    pub async fn run(&mut self) -> Result<()> {
        let initial_changes = openspec::list_changes(&self.openspec_cmd).await?;

        // Early filter by target_changes
        let filtered_initial = if let Some(targets) = &self.target_changes {
            let mut found = Vec::new();
            for target in targets {
                if let Some(change) = initial_changes.iter().find(|c| &c.id == target) {
                    found.push(change.clone());
                } else {
                    warn!("Specified change '{}' not found, skipping", target);
                }
            }
            found
        } else {
            initial_changes
        };

        if filtered_initial.is_empty() {
            info!("No changes found");
            return Ok(());
        }

        // Snapshot now only contains target changes
        let snapshot_ids: HashSet<String> = filtered_initial.iter().map(|c| c.id.clone()).collect();
        info!(
            "Captured snapshot of {} changes: {:?}",
            snapshot_ids.len(),
            snapshot_ids
        );
        // ...
    }
}
```

### 3. src/main.rs

```rust
let mut orchestrator = Orchestrator::new(&args.openspec_cmd, args.change, args.config)?;
// No change needed - args.change is now Option<Vec<String>>
```

## Backward Compatibility

- Single change: `--change foo` still works (parsed as `vec!["foo"]`)
- Multiple changes: `--change foo,bar,baz` now supported
- No change flag: behavior unchanged (all changes processed)

## Error Handling

| Scenario | Behavior |
|----------|----------|
| `--change existing` | Process only `existing` |
| `--change nonexistent` | Warn and exit with "No changes found" |
| `--change a,nonexistent,b` | Warn about `nonexistent`, process `a` and `b` |
| No `--change` | Process all changes (current behavior) |
