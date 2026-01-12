## 1. Implementation
- [x] 1.1 Update CLI/config default to auto-assign port (port 0)
- [x] 1.2 Bind port 0 when unspecified, get actual port via listener.local_addr()
- [x] 1.3 Log actual bind address/port after binding
- [x] 1.4 Update README and CLI help documentation
- [x] 1.5 Add auto-assign port tests

## 2. Validation
- [x] 2.1 `cargo test`
- [x] 2.2 `cargo fmt`
- [x] 2.3 `cargo clippy -- -D warnings`
