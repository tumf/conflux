//! Event definitions and sending helpers for parallel execution.
//!
//! This module now re-exports ExecutionEvent from the unified events module
//! and provides compatibility aliases.

// Re-export the unified event type
pub use crate::events::{send_event, ExecutionEvent};

// Type alias for backward compatibility
pub type ParallelEvent = ExecutionEvent;
