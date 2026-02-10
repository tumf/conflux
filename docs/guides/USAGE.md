# OpenSpec Orchestrator Usage Examples

## Quick Start

### Basic Usage

Run the orchestrator to process all pending changes:

```bash
cflx run
```

This will:
1. List all changes via `openspec list`
2. Analyze dependencies and select the next change
3. Apply changes using `opencode run "/openspec-apply <id>"`
4. Archive completed changes using `openspec archive <id>`
5. Repeat until all changes are processed

### Dry Run

Preview what the orchestrator would do without executing:

```bash
cflx run --dry-run
```

Output:
```
[00:00:01] ████████████████████░░░░░░░░░░░░░░░ 2/5 Overall progress
  [████████████████░░░░░░░░░░░░░░░░░░░░] 4/10 add-feature-x (40.0%)

[DRY RUN] Would apply: add-feature-x
[DRY RUN] Would archive: fix-bug-y
```

### Process Specific Change

Work on a single change:

```bash
cflx run --change add-feature-x
```

This focuses only on `add-feature-x`, ignoring other changes.

## Advanced Usage

### Custom Binary Paths

If `opencode` or `openspec` are not in your PATH:

```bash
cflx run \
  --opencode-path ~/bin/opencode \
  --openspec-path ~/bin/openspec
```

### Check Status

View current orchestration state:

```bash
cflx status
```

Output:
```
=== Orchestrator Status ===
Started at: 2026-01-08 15:00:00 UTC
Last update: 2026-01-08 15:45:00 UTC
Total iterations: 5

Current change: "add-feature-x"

Processed changes: 2
  - add-feature-x
  - refactor-z

Archived changes: 1
  - fix-bug-y

Failed changes: 0
```

### Reset State

Clear orchestration state to start fresh:

```bash
# With confirmation prompt
cflx reset

# Skip confirmation
cflx reset --yes
```

## Workflow Examples

### Example 1: Automated Full Run

```bash
# Start orchestration
cflx run

# Check progress in another terminal
watch -n 5 cflx status
```

### Example 2: Step-by-Step Processing

```bash
# Process first change
cflx run --change change-1

# Verify completion
openspec list

# Process second change
cflx run --change change-2

# Check final status
cflx status
```

### Example 3: Recovery from Failure

```bash
# Run orchestrator
cflx run

# If interrupted or failed, check status
cflx status

# Resume (state is automatically loaded)
cflx run
```

### Example 4: Development Workflow

```bash
# Dry run to see execution plan
cflx run --dry-run

# Review changes manually
openspec list

# Execute for real
cflx run

# Monitor progress
tail -f .opencode/orchestrator-state.json
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
        run: |
          cargo install --path cflx

      - name: Run orchestrator
        run: |
          cflx run
        env:
          RUST_LOG: info

      - name: Upload state
        if: always()
        uses: actions/upload-artifact@v3
        with:
          name: orchestrator-state
          path: .opencode/orchestrator-state.json
```

### Docker

```dockerfile
FROM rust:1.75 as builder
WORKDIR /app
COPY cflx ./
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
RUST_LOG=debug cflx run --dry-run 2>&1 | tee debug.log
```

### Verbose Output

```bash
RUST_LOG=trace cflx run --change test-change
```

### Check Logs

```bash
# View orchestrator state
cat .opencode/orchestrator-state.json | jq .

# Check OpenCode logs
ls -la ~/.opencode/logs/
```

## Best Practices

### 1. Always Start with Dry Run

```bash
# See what would happen
cflx run --dry-run

# If looks good, execute
cflx run
```

### 2. Monitor Progress

Use a split terminal:
```bash
# Terminal 1
cflx run

# Terminal 2
watch -n 2 'openspec list && echo && cflx status'
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

### 4. Regular State Checks

```bash
# Before starting
openspec list

# After completion
cflx status
```

## Tips

- **Performance**: The orchestrator processes changes sequentially to respect dependencies
- **Recovery**: State is saved after each iteration, so interruptions are safe
- **Debugging**: Use `RUST_LOG=debug` for detailed execution logs
- **Safety**: Dry run is your friend - always test first
- **Monitoring**: State file (`.opencode/orchestrator-state.json`) is human-readable JSON

## Common Patterns

### Pattern 1: Nightly Automation

```bash
#!/bin/bash
# nightly-orchestrator.sh

cd /path/to/project
cflx run --dry-run > /tmp/orchestrator-plan.txt

if [ $? -eq 0 ]; then
  cflx run
  echo "Orchestration completed" | mail -s "OpenSpec Orchestrator" admin@example.com
fi
```

### Pattern 2: Selective Processing

```bash
# Process only high-priority changes
openspec list | grep -E 'urgent|critical' | \
  cut -d' ' -f3 | \
  xargs -I {} cflx run --change {}
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
