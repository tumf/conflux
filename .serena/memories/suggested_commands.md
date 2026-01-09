# Suggested Commands

## Development Commands

### Build
```bash
cargo build            # Debug build
cargo build --release  # Release build
```

### Test
```bash
cargo test             # Run all tests
cargo test -- --nocapture  # Run tests with output
cargo test <test_name> # Run specific test
```

### Coverage
```bash
# Install cargo-llvm-cov if not present
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --all-features

# Generate HTML report
cargo llvm-cov --all-features --html

# Generate coverage for specific test
cargo llvm-cov --all-features -- <test_name>
```

### Lint and Format
```bash
cargo fmt              # Format code
cargo clippy           # Run linter
cargo clippy -- -D warnings  # Strict linting
```

### Run
```bash
cargo run              # Run TUI
cargo run -- run       # Run orchestration
cargo run -- run --dry-run  # Dry run
RUST_LOG=debug cargo run -- run --dry-run  # With logging
```

## System Commands (macOS/Darwin)
- `ls`, `cd`, `grep`, `find` - Standard Unix commands
- `which` - Find command location
- `open` - Open files/directories
