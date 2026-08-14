//! Test-only helpers shared across modules.

use std::sync::{Mutex, OnceLock};

/// Global mutex to serialize tests that mutate process-global state.
///
/// In particular, many tests change the current working directory via
/// `std::env::set_current_dir`, which is process-global and will race when
/// Rust tests run in parallel.
pub fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Global mutex to serialize tests that install a capturing tracing subscriber.
///
/// `tracing::subscriber::set_default` installs the subscriber thread-locally,
/// but the *max-level hint* it publishes is process-global: while a WARN-only
/// capture guard is held on any thread, `info!` callsites are disabled on every
/// thread. Two capture tests that overlap therefore silently steal each other's
/// records — a WARN-scoped capture blanks the INFO events another test is
/// asserting on, with no error and no ordering the test itself can see.
///
/// Every test that installs a capturing subscriber holds this lock for the
/// lifetime of its guard, so at most one is ever live. It is a
/// [`tokio::sync::Mutex`] because those guards are held across `.await` points,
/// and it is runtime-independent so tests on separate `#[tokio::test]` runtimes
/// still exclude each other.
pub fn tracing_capture_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// A process-global subscriber that records nothing but decides nothing either.
///
/// Its whole purpose is `register_callsite`: returning [`Interest::sometimes`]
/// keeps every callsite in the "ask each time" state, so a thread-local capture
/// subscriber installed later still gets consulted.
struct AlwaysAskSubscriber;

impl tracing::Subscriber for AlwaysAskSubscriber {
    fn register_callsite(
        &self,
        _metadata: &'static tracing::Metadata<'static>,
    ) -> tracing::subscriber::Interest {
        tracing::subscriber::Interest::sometimes()
    }

    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        false
    }

    /// INFO is the most verbose level any capture test inspects. Keeping the
    /// hint here rather than at `None` leaves `debug!`/`trace!` statically
    /// disabled, so the untouched majority of the suite pays nothing.
    fn max_level_hint(&self) -> Option<tracing::level_filters::LevelFilter> {
        Some(tracing::level_filters::LevelFilter::INFO)
    }

    fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::Id {
        tracing::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::Id, _values: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _span: &tracing::Id, _follows: &tracing::Id) {}
    fn event(&self, _event: &tracing::Event<'_>) {}
    fn enter(&self, _span: &tracing::Id) {}
    fn exit(&self, _span: &tracing::Id) {}
}

/// Make thread-local tracing capture reliable under a parallel test suite.
///
/// Callsite *interest* is cached process-globally the first time a callsite is
/// reached, using whatever subscriber that thread happened to have — and a
/// thread with no subscriber at all answers [`Interest::never`], which disables
/// that callsite permanently, on every thread. With hundreds of tests exercising
/// the same code concurrently, an `info!` a capture test asserts on can be
/// silently switched off before that test ever starts. The symptom is an
/// assertion that passes in isolation and fails in the full suite purely on
/// ordering.
///
/// Two things together make it deterministic:
///
/// 1. a global [`AlwaysAskSubscriber`], so a bare thread can never cache
///    `never`; and
/// 2. a cache rebuild against *this* test's subscriber, which clears any
///    `never` a WARN-scoped capture cached earlier.
///
/// Call it immediately after installing a capturing subscriber, while holding
/// [`tracing_capture_lock`].
pub fn refresh_tracing_interest() {
    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        // A global default may legitimately already exist; the rebuild below is
        // what this function owes its caller either way.
        let _ = tracing::subscriber::set_global_default(AlwaysAskSubscriber);
    });
    tracing::callsite::rebuild_interest_cache();
}
