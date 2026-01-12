# Implementation Tasks

## 1. Dependencies and Project Setup
- [x] 1.1 Add `axum` dependency to Cargo.toml
- [x] 1.2 Add `tower` and `tower-http` for middleware support
- [x] 1.3 Add `tokio` broadcast channel feature if not already enabled
- [x] 1.4 Add optional feature flag `web-monitoring` in Cargo.toml
- [x] 1.5 Create `src/web/` module directory structure

## 2. CLI Integration
- [x] 2.1 Add `--web` flag to CLI arguments in `src/cli.rs`
- [x] 2.2 Add `--web-port` flag with default value 8080
- [x] 2.3 Add `--web-bind` flag with default value "127.0.0.1"
- [x] 2.4 Update configuration struct to include web settings

## 3. Web Server Core
- [x] 3.1 Create `src/web/mod.rs` with server initialization logic
- [x] 3.2 Implement graceful shutdown on Ctrl+C signal
- [x] 3.3 Add CORS middleware for local development
- [x] 3.4 Add logging middleware for HTTP requests
- [x] 3.5 Set up router with API and static file routes

## 4. REST API Implementation
- [x] 4.1 Create `src/web/api.rs` with API handlers
- [x] 4.2 Implement `GET /api/health` health check endpoint
- [x] 4.3 Implement `GET /api/state` full state endpoint
- [x] 4.4 Implement `GET /api/changes` list changes endpoint
- [x] 4.5 Implement `GET /api/changes/:id` single change detail endpoint
- [x] 4.6 Add error handling and JSON serialization

## 5. WebSocket Support
- [x] 5.1 Create `src/web/websocket.rs` with WebSocket handler
- [x] 5.2 Set up tokio broadcast channel for state updates
- [x] 5.3 Implement WebSocket connection upgrade handler
- [x] 5.4 Implement WebSocket message broadcasting logic
- [x] 5.5 Add proper error handling and connection cleanup

## 6. State Broadcasting
- [x] 6.1 Create `src/web/state.rs` for state observation
- [x] 6.2 Modify orchestrator to notify web server on state changes
- [x] 6.3 Implement state diff calculation to minimize message size
- [x] 6.4 Add Arc<RwLock> wrapper for thread-safe state access
- [x] 6.5 Test concurrent state access from multiple WebSocket clients

## 7. Frontend Dashboard
- [x] 7.1 Create `web/index.html` with basic dashboard layout
- [x] 7.2 Create `web/style.css` with responsive styling
- [x] 7.3 Create `web/app.js` with WebSocket client logic
- [x] 7.4 Implement change list rendering with progress bars
- [x] 7.5 Implement task status visualization
- [x] 7.6 Add auto-reconnect logic for WebSocket disconnections
- [x] 7.7 Add loading states and error messages in UI

## 8. Static File Serving
- [x] 8.1 Embed static files in binary using `include_str!` macro
- [x] 8.2 Implement `GET /` handler to serve dashboard HTML
- [x] 8.3 Implement `GET /assets/*` handler for CSS/JS files
- [x] 8.4 Set correct Content-Type headers for each file type

## 9. Integration
- [x] 9.1 Update `src/main.rs` to initialize web server when flag is set
- [x] 9.2 Ensure web server runs in background while orchestration continues
- [x] 9.3 Add graceful shutdown coordination between TUI and web server
- [x] 9.4 Test interaction between TUI mode and web monitoring

## 10. Testing
- [x] 10.1 Add unit tests for API handlers
- [x] 10.2 Add unit tests for WebSocket message broadcasting
- [x] 10.3 Add integration test for HTTP server startup
- [x] 10.4 Add integration test for state synchronization
- [x] 10.5 Test error scenarios (port in use, invalid requests)
- [x] 10.6 Manual testing with multiple browser tabs

## 11. Documentation
- [x] 11.1 Update README.md with web monitoring usage instructions
- [x] 11.2 Document CLI flags and configuration options
- [x] 11.3 Add example screenshots of web dashboard
- [x] 11.4 Document API endpoints in developer docs
- [x] 11.5 Add troubleshooting section for common issues

## 12. Error Handling and Edge Cases
- [x] 12.1 Handle port already in use error gracefully
- [x] 12.2 Handle malformed JSON in state file
- [x] 12.3 Handle WebSocket client disconnections properly
- [x] 12.4 Handle concurrent state updates correctly
- [x] 12.5 Add timeout for WebSocket connections

## 13. Performance Optimization
- [x] 13.1 Benchmark state serialization performance
- [x] 13.2 Optimize WebSocket message size with state diffs
- [x] 13.3 Add connection limit for WebSocket clients
- [x] 13.4 Test with large number of changes (100+)

## 14. Final Validation
- [x] 14.1 Run `cargo fmt` and `cargo clippy`
- [x] 14.2 Run full test suite with `cargo test`
- [x] 14.3 Verify web server works in both run and TUI modes
- [x] 14.4 Verify graceful shutdown on Ctrl+C
- [x] 14.5 Final end-to-end test with real orchestration workflow
