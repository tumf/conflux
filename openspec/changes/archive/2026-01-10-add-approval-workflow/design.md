# Design: Add Approval Workflow

## Architecture Overview

The approval workflow introduces a file-based approval mechanism integrated into the CLI, TUI, and orchestrator components.

```
┌─────────────────────────────────────────────────────────────┐
│                    Approval Workflow                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  openspec/changes/{change_id}/                              │
│  ├── proposal.md          ─┐                                │
│  ├── design.md             │ ← Hashed for approval          │
│  ├── specs/**/*.md        ─┘                                │
│  ├── tasks.md              ← Excluded from hash validation  │
│  └── approved              ← Contains MD5 manifest          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Approved File Format

The `approved` file is a simple text file with MD5 checksums in `md5sum` output format:

```
47dadc8fb73c2d2ec6b19c0de0d71094  openspec/changes/{change_id}/proposal.md
c1fce89931c1142dd06f67a03059619d  openspec/changes/{change_id}/specs/cli/spec.md
ba74d36d6cdc1effcae37cfed4f97e19  openspec/changes/{change_id}/design.md
```

### File Discovery Rules

1. Scan `openspec/changes/{change_id}/` recursively for `*.md` files
2. Exclude `tasks.md` from the manifest
3. Sort files alphabetically for consistent ordering
4. Store relative paths from project root

## Validation Algorithm

```rust
fn is_approved(change_id: &str) -> bool {
    let approved_path = format!("openspec/changes/{}/approved", change_id);

    // 1. Check if approved file exists
    if !Path::new(&approved_path).exists() {
        return false;
    }

    // 2. Parse approved manifest
    let manifest = parse_approved_file(&approved_path)?;

    // 3. Get current file list (excluding tasks.md)
    let current_files = discover_md_files(change_id)
        .filter(|f| !f.ends_with("tasks.md"));

    // 4. Compare file lists (excluding tasks.md from both)
    let manifest_files: HashSet<_> = manifest.iter()
        .filter(|(path, _)| !path.ends_with("tasks.md"))
        .map(|(path, _)| path.clone())
        .collect();

    let current_set: HashSet<_> = current_files.collect();

    if manifest_files != current_set {
        return false; // File list mismatch
    }

    // 5. Verify hashes for all files in manifest (except tasks.md)
    for (path, expected_hash) in manifest {
        if path.ends_with("tasks.md") {
            continue;
        }
        let actual_hash = compute_md5(&path)?;
        if actual_hash != expected_hash {
            return false; // Hash mismatch
        }
    }

    true
}
```

## Component Integration

### CLI Module (`cli.rs`)

New subcommand structure:

```rust
#[derive(Subcommand, Debug)]
pub enum Commands {
    // ... existing commands ...

    /// Manage change approval status
    Approve(ApproveArgs),
}

#[derive(Parser, Debug)]
pub struct ApproveArgs {
    #[command(subcommand)]
    pub action: ApproveAction,
}

#[derive(Subcommand, Debug)]
pub enum ApproveAction {
    /// Approve a change (create approved file)
    Set { change_id: String },
    /// Unapprove a change (remove approved file)
    Unset { change_id: String },
    /// Check approval status
    Status { change_id: String },
}
```

### OpenSpec Module (`openspec.rs`)

Extend `Change` struct and add approval functions:

```rust
pub struct Change {
    pub id: String,
    pub completed_tasks: u32,
    pub total_tasks: u32,
    pub last_modified: String,
    pub is_approved: bool,  // NEW
}

/// Create approved file for a change
pub fn approve_change(change_id: &str) -> Result<()>;

/// Remove approved file for a change
pub fn unapprove_change(change_id: &str) -> Result<()>;

/// Check if a change is approved (file exists and hashes match)
pub fn check_approval(change_id: &str) -> Result<bool>;

/// Get list of files to be hashed for approval
pub fn get_approval_files(change_id: &str) -> Result<Vec<PathBuf>>;
```

### TUI Module (`tui.rs`)

Extend `ChangeState` and add UI elements:

```rust
pub struct ChangeState {
    // ... existing fields ...
    pub is_approved: bool,  // NEW
}

// Key bindings in selection mode:
// '@' - Toggle approval status for selected change
// Visual: Show "@" badge for approved changes
```

### Orchestrator Module (`orchestrator.rs`)

Queue filtering logic:

```rust
// TUI startup: Auto-queue approved changes
fn load_initial_queue(&self) -> Vec<String> {
    let changes = list_changes_native()?;
    changes
        .iter()
        .filter(|c| c.is_approved)
        .map(|c| c.id.clone())
        .collect()
}

// CLI run: Filter queue by approval status
fn process_queue(&self, change_ids: Option<Vec<String>>) -> Result<()> {
    let queue = match change_ids {
        Some(ids) => {
            // Warn and skip unapproved changes
            ids.into_iter()
                .filter(|id| {
                    let approved = check_approval(id).unwrap_or(false);
                    if !approved {
                        warn!("Skipping unapproved change: {}", id);
                    }
                    approved
                })
                .collect()
        }
        None => self.load_initial_queue(),
    };
    // ... process queue
}
```

## Error Handling

| Error Case | Behavior |
|------------|----------|
| Approved file missing | `is_approved = false` |
| Approved file unparseable | `is_approved = false`, log warning |
| File in manifest missing | `is_approved = false` |
| Hash mismatch | `is_approved = false` |
| IO error reading file | Return error, do not proceed |

## Performance Considerations

- MD5 hashing is fast for typical specification files (<1MB)
- File discovery is already performed during `list_changes_native()`
- Approval check adds minimal overhead per change

## Security Notes

- MD5 is used for integrity checking, not cryptographic security
- Approval files can be manually edited (trust-based system)
- No protection against malicious actors with file system access
