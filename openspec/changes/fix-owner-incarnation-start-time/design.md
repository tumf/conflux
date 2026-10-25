# Design: one process birth, two projections

## Source evidence and publication order

At `80956e9a`, `main` runs owner preflight and repository lock acquisition
before dispatching either TUI or `run` (`src/main.rs:948`). Acquisition takes the
OS lock and writes metadata immediately, initially without endpoints. `install`
retains the lock in `ACTIVE_LOCK` for the process lifetime.

`start_local_api` constructs `WebState` -> `RemoteControlRuntime` -> `Projection`
(`src/main.rs:428`, `src/web/state.rs:551-565`,
`src/web/remote_control_api/mod.rs:306-317`). The projection currently samples a
second clock value. `start_listeners` builds one shared router, starts requested
listeners, then publishes endpoints only after every bind succeeded
(`src/web/mod.rs:422-512`). A UDS request can already arrive before endpoint
publication finishes; identity must therefore be final at construction.

`launch_tui` binds the API before upstream initialization, lifecycle adapters,
and `run_tui` (`src/main.rs:180-259`). Only later does the runner bind shared
state, marks, runtime, and repo root (`src/tui/runner.rs:555-568`). A fix in
`set_repo_root` or other late TUI binding would leave the first response wrong.
The `run` entrypoint uses the same early API constructor.

## Chosen implementation

Add `pub fn process_started_at() -> &'static str` in `src/ids.rs`, backed by a
module-private `OnceLock<String>`. Its initializer is exactly one
`chrono::Utc::now().to_rfc3339()` per OS process. The first production read is
owner metadata creation after successful lock acquisition; if no lock is
installed under an existing bypass/error path, first projection construction
may initialize it instead. That preserves existing startup policy.

Replace the two independent samples with
`crate::ids::process_started_at().to_owned()`. Both call sites must reach the
helper through `crate::ids` in the same crate; the binary compiles its own
`mod ids`, so mixing that with `conflux::ids` would create a second static and
reintroduce the defect. Do not move `instance_id`
generation: each production projection still creates its existing random
128-bit hexadecimal ID once. PID still comes from `std::process::id()` in both
surfaces. Do not add identity to the on-disk schema or change RFC 3339 spelling
or precision. `Projection::with_registry` must use the shared helper too, so
`new`, `default`, and test construction cannot diverge.

The shared value is authoritative only for the process's **reported birth**;
it is neither kernel process creation time nor lock authority. It is immutable,
not a workflow input, and discarded by process exit. Every replacement samples
its own clock, never imports an old record. Wall-clock ordering/uniqueness is
not promised; the existing random `instance_id` still distinguishes incarnations
for replay and command contracts. A process-global birth does not globalize the
projection, revision counters, command registries, or random instance IDs:
separate synthetic projections in one test process keep their current isolation.

No async coordination or production wait is needed: `OnceLock` initializes
synchronously before either publication. States remain: no owner; metadata with
birth but no endpoints; listeners binding; endpoints published; later TUI
wiring. An early or missing endpoint is not readiness; an API response that is
already reachable must nevertheless carry the final birth. Metadata I/O remains
best-effort, and partial listener failure keeps its existing rollback. There is
no reader of the old owner file in the birth path and no repair command.

## Rejected alternatives

- Read the owner JSON into the API: makes stale, malformed, or attacker-modified
  diagnostic content an identity source; introduces filesystem coupling.
- Copy the API timestamp back to metadata after listener bind: mutates an already
  published birth and leaves a race before correction.
- Repair in TUI `set_repo_root`: misses the reachable pre-TUI API window.
- Use an installed-lock optional accessor with a fresh-clock fallback: introduces
  unnecessary lock-to-web coupling and two possible birth sources. The existing
  ungated `ids` module can supply one value without changing constructor chains.
- Add tolerance, timestamp truncation, PID-only adoption, or an endpoint schema
  redesign: changes consumer safeguards or independent contracts, not this bug.

## Verification design (future implementation, not executed at proposal time)

Use the `owner_incarnation` test-name prefix for every new regression. Both
focused commands must report tests actually run; zero selected tests is not
acceptance. Keep tests deterministic and small (the repository's under-one-second
normal-runtime target); use event handshakes and generous finite hang guards,
never elapsed-time precision assertions or sleeps to manufacture equal clocks.

### Unit and router gate: `owner-incarnation-local`

- `src/ids.rs`: nonempty parseable RFC 3339; repeated and concurrent helper reads
  return the same stored string. Do not reset global state between tests.
- `src/repo_lock.rs` and API read tests: acquire a real lock in a disposable Git
  fixture without installing it globally, build the production `WebState`/router,
  and compare serialized metadata with the actual `/api/v2/instance` response.
  Cover projection-before-lock and lock-before-projection order, asserting
  byte equality with `ids::process_started_at()` in both. These cases do not
  assert which test initialized the global: it runs once per test binary and
  must not be reset for testing. Production first-read order remains a source
  review property of `main` startup. Endpoint
  publication must preserve PID/birth; later state/root binding and projection
  updates plus repeated GETs must preserve birth and `instance_id`.
- Seed a disposable stale/malformed metadata record and verify acquisition's
  reported birth comes from the shared in-memory value, not that content. Retain
  existing metadata parsing, OS-lock conflict, and publication tests unchanged.
- Apply a test-only consumer-shaped equality oracle to the real serialized pair:
  equal PID + exact birth succeeds; independently changed PID or birth fails;
  mutate one fractional nanosecond and ensure rejection. Mutations are only
  in-memory copies, not writes to a live record. This documents the existing
  consumer contract, not a new producer adoption API.

### Process gate: `owner-incarnation-startup`

Extend `tests/run_exit_tests.rs` using `CARGO_BIN_EXE_cflx`, disposable main Git
worktrees, isolated config/HOME/state paths, and bounded child cleanup/reaping.
Do not install a global `ACTIVE_LOCK` in the test parent or use a shared owner.

For default and explicit TUI startup, use the existing local upstream-fixture
pattern plus a child-only Git shim: delegate Git normally, but signal a pipe
when `fetch --prune` is reached and wait on its release pipe. This holds the
**real** TUI after listener publication and before `AppState` creation without
any production test switch, network service, or lifecycle/AI work. At that
handshake, read the real owner file and issue two real UDS
`GET /api/v2/instance` requests. Assert exact PID/birth equality and stable,
nonempty `instance_id` before allowing initialization onward. A no-PTY child may
exit at terminal setup after release; this is not used as evidence of late TUI
binding (the router gate covers that separately). The test guard releases the
gate on drop before waiting for or terminating/reaping the `cflx` child. The
child-only shim polls its release gate with a 30-second maximum so no failure
can strand it indefinitely. Prefer normal owner exit so it reaps its shim; if
forced owner termination is required, the orphan shim exits at that ceiling and
is reaped by the OS reaper, not by a false test-parent `wait` claim. This ceiling
is only a hang safeguard, never a timing assertion.

Retain the first owner's record without editing it, then start a second isolated
child against that same repository and repeat. Assert a newly sampled birth and
new `instance_id` under the fixture's ordinary advancing host clock (not a
claim of timestamp uniqueness under clock rollback), and verify a saved
first-owner pair and a same-PID/wrong-birth
copy are refused by the equality oracle. Cover `run` with the existing
process/socket harness and a pipe-gated local command fixture so its shared
startup path is also compared while alive; no real AI or remote is needed.
For UDS+TCP, extend the existing real-listener coverage to compare both complete
instance responses rather than only IDs; neither transport may mint a birth.

The startup test is a regression against both the two-clock bug and a late
TUI-only repair. A fixture barrier observes the already-correct production
ordering; it is not a production readiness workaround.

## Operational handoff, not a completion gate

The consumer currently compares PID and `started_at` by exact equality and
obtains `instance_id` from the API. Preserve that contract. Its distinct owner
`endpoints` map/array mismatch belongs to the separately owned server proposal.
Producer-local tests do not claim to execute the external consumer or establish
CFSRV-1 live acceptance. That later operation requires the reviewed producer
implementation, a newly pinned binary and newly authorized owner, consumer
schema alignment, then independent read-only identity confirmation. Never
rewrite the preserved owner's old record to make that later check pass.
