//! Clipboard boundary for the TUI.
//!
//! The TUI never talks to the OS clipboard directly. Everything goes through
//! [`Clipboard`], so automated tests can assert exactly what would have been
//! copied without mutating the developer's real clipboard, and so the single
//! production implementation stays the only place that knows about the platform
//! clipboard API. There is deliberately no shell-command fallback: a failure is
//! reported to the operator instead of being retried through a subprocess.
//!
//! This is presentation-support state only. Copying never influences retry,
//! scheduling, acceptance, archive, or any other workflow-control decision.

use std::sync::Arc;

/// Minimal write-only clipboard boundary.
pub trait Clipboard: Send + Sync + std::fmt::Debug {
    /// Place `text` on the clipboard, or return an operator-facing reason why not.
    fn set_text(&self, text: &str) -> Result<(), String>;
}

/// The real OS clipboard.
#[derive(Debug, Default)]
pub struct SystemClipboard;

impl Clipboard for SystemClipboard {
    fn set_text(&self, text: &str) -> Result<(), String> {
        let mut clipboard = arboard::Clipboard::new().map_err(|err| err.to_string())?;
        clipboard
            .set_text(text.to_string())
            .map_err(|err| err.to_string())
    }
}

/// The clipboard a freshly constructed `AppState` uses.
pub fn default_clipboard() -> Arc<dyn Clipboard> {
    Arc::new(SystemClipboard)
}

#[cfg(test)]
pub(crate) mod test_doubles {
    use super::Clipboard;
    use std::sync::Mutex;

    /// Records what would have been copied instead of touching the real clipboard.
    #[derive(Debug, Default)]
    pub(crate) struct RecordingClipboard {
        copies: Mutex<Vec<String>>,
    }

    impl RecordingClipboard {
        /// Every text handed to the clipboard, in call order.
        pub(crate) fn copies(&self) -> Vec<String> {
            self.copies.lock().expect("clipboard mutex").clone()
        }
    }

    impl Clipboard for RecordingClipboard {
        fn set_text(&self, text: &str) -> Result<(), String> {
            self.copies
                .lock()
                .expect("clipboard mutex")
                .push(text.to_string());
            Ok(())
        }
    }

    /// Always refuses, so failure feedback can be exercised deterministically.
    #[derive(Debug)]
    pub(crate) struct FailingClipboard {
        pub(crate) reason: String,
    }

    impl FailingClipboard {
        pub(crate) fn new(reason: impl Into<String>) -> Self {
            Self {
                reason: reason.into(),
            }
        }
    }

    impl Clipboard for FailingClipboard {
        fn set_text(&self, _text: &str) -> Result<(), String> {
            Err(self.reason.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_doubles::{FailingClipboard, RecordingClipboard};
    use super::Clipboard;

    /// The doubles are the only clipboard automated tests ever reach. They both
    /// answer from memory, so no test run can write to the developer's real
    /// clipboard — [`super::SystemClipboard`] is reachable only from
    /// [`super::default_clipboard`], which production wiring uses.
    #[test]
    fn test_doubles_record_or_refuse_without_touching_the_os_clipboard() {
        let recording = RecordingClipboard::default();
        assert!(recording.set_text("payload").is_ok());
        assert!(recording.set_text("second").is_ok());
        assert_eq!(
            recording.copies(),
            vec!["payload".to_string(), "second".to_string()]
        );

        let failing = FailingClipboard::new("no clipboard provider");
        assert_eq!(
            failing.set_text("payload"),
            Err("no clipboard provider".to_string())
        );
    }
}
