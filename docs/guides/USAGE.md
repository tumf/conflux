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

Use `@` to approve changes, `Space` to select them, then `F5` to start processing.

### 4. Run Headless (Non-Interactive)

Process all pending changes without the TUI:

```bash
cflx run
```

This will:
1. List all changes via `openspec list`
2. Analyze dependencies and select the next change
3. Apply changes using the configured AI agent command
4. Archive completed changes
5. Repeat until all changes are processed

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

### Preview Parallel Execution Groups

```bash
cflx run --parallel --dry-run
```

### Enable Parallel Execution

```bash
# Auto-detect VCS backend
cflx run --parallel

# Limit concurrent workspaces
cflx run --parallel --max-concurrent 5
```

## Workflow Examples

### Example 1: Automated Full Run (Headless)

```bash
# Run orchestration loop
cflx run
```

### Example 2: Step-by-Step Processing

```bash
# Process first change
cflx run --change change-1

# Verify changes
openspec list

# Process second change
cflx run --change change-2
```

### Example 3: Resume After Interruption

```bash
# Run orchestrator (interrupted mid-run)
cflx run

# Resume (parallel mode automatically reuses existing workspaces)
cflx run --parallel
```

### Example 4: TUI Workflow

```bash
# Launch TUI
cflx

# In TUI:
# 1. Press @ to approve a change
# 2. Press Space to select it for processing
# 3. Press F5 to start
# 4. Monitor progress in real time
# 5. Press Ctrl+C to quit
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
name: OpenSpec Orchestrator

on:
  schedule:
    - cron: '0 */4 * * *'  # Every 4 hours
  workflow_dispatch:

jobs:
  orchestrate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install dependencies
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
# Launch TUI (most user-friendly)
cflx

# Approve, select, and start changes interactively
```

### 2. Use Headless Mode for Automation

```bash
# Fully non-interactive run
cflx run
```

### 3. Incremental Processing

For safety, process one change at a time:
```bash
for change in $(openspec list | grep -oP '^\s*-\s+\K[^\s]+'); do
  cflx run --change "$change"
  if [ $? -ne 0 ]; then
    echo "Failed on $change"
    break
  fi
done
```

### 4. Preview Before Running (Parallel Mode)

```bash
# Preview parallelization groups without executing
cflx run --parallel --dry-run

# If looks good, execute
cflx run --parallel
```

## Tips

- **TUI**: Default mode — use `cflx` for interactive orchestration
- **Headless**: Use `cflx run` for CI/CD and automated pipelines
- **Configuration**: Run `cflx init` to generate a `.cflx.jsonc` config file
- **Parallel**: Use `cflx run --parallel` to process independent changes simultaneously
- **Debugging**: Use `RUST_LOG=debug` for detailed execution logs
- **Web monitoring**: Add `--web` to enable the HTTP dashboard

## Common Patterns

### Pattern 1: Nightly Automation

```bash
#!/bin/bash
# nightly-orchestrator.sh

cd /path/to/project
cflx run

if [ $? -eq 0 ]; then
  echo "Orchestration completed" | mail -s "OpenSpec Orchestrator" admin@example.com
fi
```

### Pattern 2: Selective Processing

```bash
# Process only specific changes
cflx run --change urgent-fix,critical-update
```

### Pattern 3: Progress Notification

```bash
cflx run
STATUS=$?

if [ $STATUS -eq 0 ]; then
  curl -X POST https://hooks.slack.com/... \
    -d '{"text":"✅ OpenSpec orchestration completed"}'
else
  curl -X POST https://hooks.slack.com/... \
    -d '{"text":"❌ OpenSpec orchestration failed"}'
fi
```
