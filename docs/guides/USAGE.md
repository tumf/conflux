# Conflux Usage Examples

## Quick Start

### Golden Path: First-Time Setup

The fastest way to get started:

```bash
# Step 1: Generate configuration for your AI agent
cflx init

# Step 2: Edit the generated .cflx.jsonc to set your agent commands
vim .cflx.jsonc

# Step 3a: Launch the interactive TUI to review and process changes
cflx

# Step 3b: Or run in headless (non-interactive) mode
cflx run
```

### Basic Usage

Launch the interactive TUI dashboard (default):

```bash
cflx
```

Or run orchestration in headless (non-interactive) mode:

```bash
cflx run
```

The `cflx run` command will:
1. List all pending changes via `openspec list`
2. Analyze dependencies and select the next change
3. Apply changes using the configured AI agent command
4. Archive completed changes
5. Repeat until all changes are processed

### Process Specific Change

Work on a single change:

```bash
cflx run --change add-feature-x
```

This focuses only on `add-feature-x`, ignoring other changes.

### Multiple Changes

Process a specific set of changes:

```bash
cflx run --change add-feature-x,fix-bug-y,refactor-z
```

## Configuration

### Generate Configuration File

```bash
# Default: Claude Code template
cflx init

# OpenCode template
cflx init --template opencode

# Codex template
cflx init --template codex

# Overwrite existing config
cflx init --force
```

Available templates: `claude` (default), `opencode`, `codex`

### Custom Configuration File

```bash
cflx run --config /path/to/config.jsonc
```

## Parallel Execution

### Preview Parallelization Plan (Dry Run)

Preview parallelization groups without executing:

```bash
cflx run --parallel --dry-run
```

### Run in Parallel Mode

```bash
# Auto-detect VCS backend (default)
cflx run --parallel

# Force Git worktrees
cflx run --parallel --vcs git

# Limit concurrent workspaces
cflx run --parallel --max-concurrent 5
```

## Web Monitoring

Enable the web monitoring server alongside the TUI or headless run:

```bash
# TUI with web monitoring (OS auto-assigns port)
cflx --web

# Headless run with web monitoring
cflx run --web

# Custom port and bind address
cflx --web --web-port 9000 --web-bind 0.0.0.0
```

Access the dashboard at `http://localhost:<port>/` (port shown in startup log).

## Workflow Examples

### Example 1: Automated Full Run

```bash
# Generate config and start orchestration
cflx init
cflx run
```

### Example 2: Interactive TUI Workflow

```bash
# Launch TUI and interactively select changes to process
cflx
# - Use @ to approve changes
# - Use Space to select changes
# - Press F5 to start processing
```

### Example 3: Step-by-Step Processing

```bash
# Process first change
cflx run --change change-1

# Verify completion
openspec list

# Process second change
cflx run --change change-2
```

### Example 4: Recovery from Interruption

```bash
# Run orchestrator
cflx run

# If interrupted, just run again - workspaces are automatically resumed
cflx run

# To force fresh start (discard existing workspaces)
cflx run --parallel --no-resume
```

### Example 5: Development Workflow

```bash
# Preview parallelization plan
cflx run --parallel --dry-run

# Review changes manually
openspec list

# Execute in parallel
cflx run --parallel
```

## Integration with CI/CD

### GitHub Actions

```yaml
name: Conflux Orchestrator

on:
  schedule:
    - cron: '0 */4 * * *'  # Every 4 hours
  workflow_dispatch:

jobs:
  orchestrate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Conflux
        run: cargo install --path .

      - name: Run orchestrator
        run: cflx run
        env:
          RUST_LOG: info
```

### Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM ubuntu:22.04
RUN apt-get update && apt-get install -y openssl ca-certificates
COPY --from=builder /app/target/release/cflx /usr/local/bin/
ENTRYPOINT ["cflx"]
```

Run:
```bash
docker build -t cflx .
docker run -v $(pwd):/workspace cflx run
```

## Troubleshooting Examples

### Debug Mode

```bash
RUST_LOG=debug cflx run 2>&1 | tee debug.log
```

### Verbose Output

```bash
RUST_LOG=trace cflx run --change test-change
```

### Check OpenSpec Changes

```bash
# List all pending changes
openspec list

# Check for spec conflicts between changes
cflx check-conflicts
```

## Best Practices

### 1. Use TUI for Interactive Work

```bash
# Launch TUI for visual progress and control
cflx
```

### 2. Use `run` for Automated Pipelines

```bash
# Headless mode for CI/CD or background execution
cflx run
```

### 3. Incremental Processing

For safety, process one change at a time:
```bash
for change in $(openspec list --json | jq -r '.[].id'); do
  cflx run --change "$change"
  if [ $? -ne 0 ]; then
    echo "Failed on $change"
    break
  fi
done
```

### 4. Monitor with Web UI

```bash
# Start with web monitoring
cflx --web

# Or headless with web
cflx run --web
# Access dashboard: http://localhost:<port>/
```

## Tips

- **Primary interface**: Use `cflx` (TUI) for interactive work; `cflx run` for automation
- **Recovery**: Parallel mode automatically resumes from interrupted workspaces
- **Debugging**: Use `RUST_LOG=debug` for detailed execution logs
- **Conflict checking**: Use `cflx check-conflicts` to find spec conflicts before processing
- **Templates**: Use `cflx init --template <claude|opencode|codex>` to match your AI agent

## Common Patterns

### Pattern 1: Nightly Automation

```bash
#!/bin/bash
# nightly-orchestrator.sh

cd /path/to/project
cflx run
STATUS=$?

if [ $STATUS -eq 0 ]; then
  echo "Orchestration completed successfully"
else
  echo "Orchestration failed with status $STATUS"
fi
```

### Pattern 2: Selective Processing

```bash
# Process specific changes by name
cflx run --change urgent-fix,critical-update
```

### Pattern 3: Parallel with Monitoring

```bash
# Run parallel with web monitoring
cflx run --parallel --web --web-bind 0.0.0.0
# Access from any device on local network
```
