//! Output handler trait for CLI and TUI modes.
//!
//! Provides a unified interface for outputting messages during orchestration.
//! CLI mode uses logging, TUI mode uses event channels.
//!
//! Note: These types are infrastructure for future CLI/TUI integration.
//! They will be used as the refactoring continues.

use tracing::{error, info, warn};

/// Trait for handling output during orchestration operations.
///
/// Implementations determine how messages are delivered to the user.
/// CLI mode typically logs to stdout/stderr, while TUI mode sends
/// events through a channel to update the UI.
pub trait OutputHandler: Send + Sync {
    /// Handle standard output from a subprocess.
    fn on_stdout(&self, line: &str);

    /// Handle standard error from a subprocess.
    fn on_stderr(&self, line: &str);

    /// Handle an informational message.
    fn on_info(&self, message: &str);

    /// Handle a warning message.
    fn on_warn(&self, message: &str);

    /// Handle an error message.
    fn on_error(&self, message: &str);

    /// Handle a success message.
    fn on_success(&self, message: &str);
}

/// Log-based output handler for CLI mode.
///
/// Outputs messages using the tracing logging framework.
#[derive(Debug, Clone, Default)]
pub struct LogOutputHandler;

impl LogOutputHandler {
    /// Create a new log-based output handler.
    pub fn new() -> Self {
        Self
    }
}

impl OutputHandler for LogOutputHandler {
    fn on_stdout(&self, line: &str) {
        info!(target: "orchestrator::output", "{}", line);
    }

    fn on_stderr(&self, line: &str) {
        warn!(target: "orchestrator::output", "{}", line);
    }

    fn on_info(&self, message: &str) {
        info!("{}", message);
    }

    fn on_warn(&self, message: &str) {
        warn!("{}", message);
    }

    fn on_error(&self, message: &str) {
        error!("{}", message);
    }

    fn on_success(&self, message: &str) {
        info!("{}", message);
    }
}

/// No-op output handler for silent operation.
///
/// Discards all output. Useful for testing or when output is not needed.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)] // Reserved for testing
pub struct NullOutputHandler;

impl NullOutputHandler {
    /// Create a new null output handler.
    #[allow(dead_code)] // Reserved for testing
    pub fn new() -> Self {
        Self
    }
}

impl OutputHandler for NullOutputHandler {
    fn on_stdout(&self, _line: &str) {}
    fn on_stderr(&self, _line: &str) {}
    fn on_info(&self, _message: &str) {}
    fn on_warn(&self, _message: &str) {}
    fn on_error(&self, _message: &str) {}
    fn on_success(&self, _message: &str) {}
}

/// Channel-based output handler for TUI mode.
///
/// Sends output through a channel using a callback.
/// This allows TUI mode to receive output as events.
#[derive(Clone)]
pub struct ChannelOutputHandler<F>
where
    F: Fn(OutputMessage) + Send + Sync,
{
    callback: F,
}

/// Messages that can be sent through the output channel.
#[derive(Debug, Clone)]
pub enum OutputMessage {
    /// Standard output line
    Stdout(String),
    /// Standard error line
    Stderr(String),
    /// Info message
    Info(String),
    /// Warning message
    Warn(String),
    /// Error message
    Error(String),
    /// Success message
    Success(String),
}

impl<F> ChannelOutputHandler<F>
where
    F: Fn(OutputMessage) + Send + Sync,
{
    /// Create a new channel-based output handler with the given callback.
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F> OutputHandler for ChannelOutputHandler<F>
where
    F: Fn(OutputMessage) + Send + Sync,
{
    fn on_stdout(&self, line: &str) {
        (self.callback)(OutputMessage::Stdout(line.to_string()));
    }

    fn on_stderr(&self, line: &str) {
        (self.callback)(OutputMessage::Stderr(line.to_string()));
    }

    fn on_info(&self, message: &str) {
        (self.callback)(OutputMessage::Info(message.to_string()));
    }

    fn on_warn(&self, message: &str) {
        (self.callback)(OutputMessage::Warn(message.to_string()));
    }

    fn on_error(&self, message: &str) {
        (self.callback)(OutputMessage::Error(message.to_string()));
    }

    fn on_success(&self, message: &str) {
        (self.callback)(OutputMessage::Success(message.to_string()));
    }
}

/// Wrapper that adds mutable operation context to another OutputHandler.
///
/// This allows the same underlying handler to be used for different operations
/// (apply, acceptance, archive, resolve) with the operation being updateable.
pub struct ContextualOutputHandler<H: OutputHandler> {
    inner: H,
    operation: std::sync::Arc<std::sync::RwLock<String>>,
}

impl<H: OutputHandler> ContextualOutputHandler<H> {
    /// Create a new contextual output handler.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying output handler
    /// * `operation` - Shared operation tracker
    pub fn new(inner: H, operation: std::sync::Arc<std::sync::RwLock<String>>) -> Self {
        Self { inner, operation }
    }

    /// Update the current operation
    #[allow(dead_code)]
    pub fn set_operation(&self, operation: impl Into<String>) {
        *self.operation.write().unwrap() = operation.into();
    }

    /// Get the current operation name
    #[allow(dead_code)]
    pub fn operation(&self) -> String {
        self.operation.read().unwrap().clone()
    }
}

impl<H: OutputHandler> OutputHandler for ContextualOutputHandler<H> {
    fn on_stdout(&self, line: &str) {
        self.inner.on_stdout(line);
    }

    fn on_stderr(&self, line: &str) {
        self.inner.on_stderr(line);
    }

    fn on_info(&self, message: &str) {
        self.inner.on_info(message);
    }

    fn on_warn(&self, message: &str) {
        self.inner.on_warn(message);
    }

    fn on_error(&self, message: &str) {
        self.inner.on_error(message);
    }

    fn on_success(&self, message: &str) {
        self.inner.on_success(message);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Test output handler that collects messages for verification.
    #[derive(Default)]
    struct TestOutputHandler {
        messages: Arc<Mutex<Vec<(String, String)>>>,
    }

    impl TestOutputHandler {
        fn new() -> Self {
            Self {
                messages: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_messages(&self) -> Vec<(String, String)> {
            self.messages.lock().unwrap().clone()
        }
    }

    impl OutputHandler for TestOutputHandler {
        fn on_stdout(&self, line: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("stdout".to_string(), line.to_string()));
        }

        fn on_stderr(&self, line: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("stderr".to_string(), line.to_string()));
        }

        fn on_info(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("info".to_string(), message.to_string()));
        }

        fn on_warn(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("warn".to_string(), message.to_string()));
        }

        fn on_error(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("error".to_string(), message.to_string()));
        }

        fn on_success(&self, message: &str) {
            self.messages
                .lock()
                .unwrap()
                .push(("success".to_string(), message.to_string()));
        }
    }

    #[test]
    fn test_test_output_handler() {
        let handler = TestOutputHandler::new();
        handler.on_stdout("stdout line");
        handler.on_stderr("stderr line");
        handler.on_info("info message");
        handler.on_warn("warn message");
        handler.on_error("error message");
        handler.on_success("success message");

        let messages = handler.get_messages();
        assert_eq!(messages.len(), 6);
        assert_eq!(
            messages[0],
            ("stdout".to_string(), "stdout line".to_string())
        );
        assert_eq!(
            messages[1],
            ("stderr".to_string(), "stderr line".to_string())
        );
        assert_eq!(
            messages[2],
            ("info".to_string(), "info message".to_string())
        );
        assert_eq!(
            messages[3],
            ("warn".to_string(), "warn message".to_string())
        );
        assert_eq!(
            messages[4],
            ("error".to_string(), "error message".to_string())
        );
        assert_eq!(
            messages[5],
            ("success".to_string(), "success message".to_string())
        );
    }

    #[test]
    fn test_null_output_handler() {
        // NullOutputHandler should not panic
        let handler = NullOutputHandler::new();
        handler.on_stdout("stdout");
        handler.on_stderr("stderr");
        handler.on_info("info");
        handler.on_warn("warn");
        handler.on_error("error");
        handler.on_success("success");
    }

    #[test]
    fn test_log_output_handler() {
        // LogOutputHandler should not panic (actual logging is tested via integration)
        let handler = LogOutputHandler::new();
        handler.on_stdout("stdout");
        handler.on_stderr("stderr");
        handler.on_info("info");
        handler.on_warn("warn");
        handler.on_error("error");
        handler.on_success("success");
    }
}
