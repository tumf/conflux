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
