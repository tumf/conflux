# Conflux Usage Examples

## Quick Start (Golden Path)

### 1. Install

```bash
cargo install --path .
```

### 2. Initialize Configuration

Generate a configuration file for your preferred AI agent:

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

### 3. Launch the TUI (Default)

The primary interface is the interactive TUI dashboard:

```bash
cflx
```

In the TUI:
- Use `Space` to select changes
- Press `F5` to start processing

### 4. Run Headless (Non-Interactive)

Process all pending changes without the TUI:

```bash
cflx run
```

The `cflx run` command will:
1. List pending changes via `openspec list`
2. Analyze dependencies and select the next change
3. Apply changes using the configured AI agent command
4. Run acceptance (if configured)
5. Archive completed changes
6. Repeat until all changes are processed

## Common Usage Patterns

### Process a Specific Change

```bash
cflx run --change add-feature-x
```

### Process Multiple Specific Changes

```bash
cflx run --change add-feature-x,fix-bug-y,refactor-z
```

### Use a Custom Configuration File

```bash
cflx run --config /path/to/config.jsonc
```

## Parallel Execution

Run independent changes in parallel using Git worktrees:

```bash
# Preview parallelization groups without executing
cflx run --parallel --dry-run

# Execute in parallel
cflx run --parallel
```

Resume behavior:

```bash
# Parallel mode automatically reuses existing workspaces
cflx run --parallel

# Force a fresh start (discard existing workspaces)
cflx run --parallel --no-resume
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

## Remote TUI (Server Mode)

If you're running the multi-project server daemon, you can point the TUI at it:

```bash
# Connect the TUI to a remote Conflux server
cflx --server http://host:9876

# With bearer-token auth
cflx --server http://host:9876 --server-token "$TOKEN"

# Or read token from environment variable
cflx --server http://host:9876 --server-token-env CFLX_SERVER_TOKEN
```

## Workflow Examples

### Example 1: Automated Full Run (Headless)

```bash
cflx run
```

### Example 2: Interactive TUI Workflow

```bash
cflx
```

### Example 3: Step-by-Step Processing

```bash
# Process first change
cflx run --change change-1

# Verify changes
openspec list

# Process second change
cflx run --change change-2
```

### Example 4: Resume After Interruption

```bash
# Run orchestrator (interrupted mid-run)
cflx run

# If interrupted, just run again - workspaces are automatically resumed
cflx run
```

### Example 5: Parallel Execution with Web Monitoring

```bash
# Run with parallel mode and web dashboard
cflx run --parallel --web

# Or use TUI with web monitoring
cflx --web
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

## Best Practices

### 1. Use the TUI for Interactive Work

```bash
cflx
```

### 2. Use Headless Mode for Automation

```bash
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

### 4. Preview Before Running (Parallel Mode)

```bash
cflx run --parallel --dry-run
cflx run --parallel
```

### 5. Monitor with Web UI

```bash
cflx --web
cflx run --web
```

### 6. Check Spec Conflicts Early

```bash
cflx check-conflicts
```

## Tips

- Default mode is the TUI: `cflx`
- Use `cflx run` for CI/CD and automated pipelines
- Use `cflx run --parallel` for independent changes
- Add `--web` for the HTTP dashboard
- Use `RUST_LOG=debug` for detailed logs

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
# Process only specific changes
cflx run --change urgent-fix,critical-update
```

### Pattern 3: Parallel with Monitoring

```bash
# Run parallel with web monitoring
cflx run --parallel --web --web-bind 0.0.0.0
```
