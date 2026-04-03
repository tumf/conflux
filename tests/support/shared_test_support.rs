//! Shared test helpers for integration tests.

use std::sync::{Mutex, MutexGuard, OnceLock};

/// Process-wide lock for tests that mutate environment variables
/// like PATH/HOME and other process-global state.
pub fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
}
