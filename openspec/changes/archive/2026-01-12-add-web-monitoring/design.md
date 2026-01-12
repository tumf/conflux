# Design: Web Monitoring Interface

## Context
The orchestrator currently tracks state in `.opencode/orchestrator-state.json` and provides TUI-based monitoring. Users need a way to monitor progress remotely via web browser, especially for long-running orchestrations or team collaboration scenarios.

## Goals / Non-Goals

**Goals:**
- Provide real-time visibility into orchestration state via HTTP
- Enable remote monitoring from any device with a browser
- Keep the feature optional and lightweight (minimal dependencies)
- Support both static snapshots (REST API) and live updates (WebSocket)

**Non-Goals:**
- Not building a full control interface (no stop/start/modify operations initially)
- Not replacing the TUI - this is complementary
- Not implementing authentication/authorization in the first iteration (local-only use case)

## Decisions

### HTTP Framework: axum
**Decision:** Use `axum` crate for HTTP server implementation.

**Rationale:**
- Tokio-native (matches existing async runtime)
- Type-safe routing and extractors
- Excellent WebSocket support
- Minimal overhead and fast performance
- Good Rust ecosystem integration

**Alternatives considered:**
- `actix-web`: More mature but uses its own runtime
- `warp`: Similar features but steeper learning curve
- `rocket`: More opinionated, less async-native

### Frontend: Vanilla HTML/CSS/JS
**Decision:** Start with simple, embedded static assets (no build step).

**Rationale:**
- No complex build toolchain needed
- Can be embedded directly in binary using `include_str!` macro
- Sufficient for MVP dashboard (change list, progress bars, task status)
- Can upgrade to React/Vue later if needed

**Alternatives considered:**
- React/Vue: Overkill for initial MVP, adds build complexity
- HTMX: Good option but requires learning new paradigm

### WebSocket Protocol
**Decision:** Send JSON messages with state deltas on every orchestrator state update.

**Message format:**
```json
{
  "type": "state_update",
  "timestamp": "2024-01-12T10:30:00Z",
  "changes": [
    {
      "id": "add-web-monitoring",
      "completed_tasks": 3,
      "total_tasks": 10,
      "status": "in_progress"
    }
  ]
}
```

### API Endpoints

**REST API (read-only):**
- `GET /api/state` - Full orchestrator state (JSON)
- `GET /api/changes` - List of all changes with progress
- `GET /api/changes/:id` - Detailed info for specific change
- `GET /api/health` - Health check endpoint

**WebSocket:**
- `ws://localhost:8080/ws` - Real-time state updates

**Static:**
- `GET /` - Dashboard HTML
- `GET /assets/*` - CSS/JS assets

### Configuration

Add to CLI flags:
```rust
#[arg(long, help = "Enable web monitoring server")]
web: bool,

#[arg(long, default_value = "8080", help = "Web server port")]
web_port: u16,

#[arg(long, default_value = "127.0.0.1", help = "Web server bind address")]
web_bind: String,
```

Add to configuration file:
```jsonc
{
  "web": {
    "enabled": false,
    "port": 8080,
    "bind": "127.0.0.1"
  }
}
```

### Module Structure

```
src/
  web/
    mod.rs         - Module root, HTTP server setup
    api.rs         - REST API handlers
    websocket.rs   - WebSocket connection handling
    state.rs       - State broadcasting logic

web/
  index.html       - Dashboard UI
  style.css        - Styling
  app.js           - Frontend logic (WebSocket client)
```

## Risks / Trade-offs

**Risk: WebSocket connection overhead**
- Mitigation: Only broadcast state updates when changes occur, not on a timer
- Mitigation: Support multiple concurrent WebSocket clients efficiently with broadcast channels

**Risk: Binary size increase**
- Mitigation: Make HTTP server optional via feature flag `web-monitoring`
- Trade-off: Users who don't need web monitoring can disable it at compile time

**Risk: Port conflicts**
- Mitigation: Make port configurable, fail gracefully with clear error message if port is in use

**Risk: State consistency during updates**
- Mitigation: Use Arc<RwLock<State>> for thread-safe state access
- Mitigation: Clone state for each HTTP request to avoid blocking

## Migration Plan

### Phase 1: MVP (This Change)
1. Add basic HTTP server with static dashboard
2. Implement REST API for state queries
3. Add WebSocket support for live updates
4. CLI flag to enable/disable

### Phase 2: Enhancements (Future)
- Add authentication (token-based or basic auth)
- Add control operations (pause/resume/cancel)
- Add dependency graph visualization
- Add historical data / logs view
- Add export functionality (CSV, JSON)

### Rollback Plan
- Feature is opt-in via CLI flag, so no impact if unused
- Can be disabled at compile time via feature flag
- No database or persistent state - just reads existing JSON state file

## Open Questions

1. **Q:** Should we support HTTPS/TLS for remote access?
   **A:** Not in MVP - local-only use case. Can add later with `rustls` if needed.

2. **Q:** Should we support multiple orchestrator instances on the same dashboard?
   **A:** Not in MVP - single instance only. Future enhancement could support this.

3. **Q:** Should we add Prometheus/metrics endpoint?
   **A:** Good idea for observability. Consider adding in Phase 2.
