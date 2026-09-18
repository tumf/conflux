//! Locally typed evaluation failures.
//!
//! Every variant is swallowed by the recorder after one bounded warning: none
//! of them may reach `AnalysisResult`, provenance, scheduler routing, blockers,
//! Acceptance, Archive, lifecycle state, or process exit status. They are typed
//! rather than stringly so a test can assert *which* degradation happened
//! without matching on message text.
//!
//! The `Display` text is deliberately path-free and content-free: it names what
//! went wrong, never which file or which change it went wrong for. That is what
//! lets a warning be emitted into the ordinary log without copying evaluation
//! data into it.

/// Result alias for evaluation-local operations.
pub type EvaluationResult<T> = std::result::Result<T, EvaluationError>;

#[derive(Debug, thiserror::Error)]
pub enum EvaluationError {
    /// The dedicated evaluation root could not be resolved from configuration.
    #[error("evaluation root could not be resolved")]
    Root(#[from] crate::config::defaults::StateRootError),

    /// The evaluation root or a project directory could not be prepared.
    #[error("evaluation directory could not be prepared")]
    Directory {
        #[source]
        source: std::io::Error,
    },

    /// The installation salt is absent, unreadable, or not trustworthy.
    #[error("installation salt unusable: {reason}")]
    Salt {
        reason: &'static str,
        #[source]
        source: Option<std::io::Error>,
    },

    /// The record could not be appended.
    #[error("evaluation record could not be appended")]
    Append {
        #[source]
        source: std::io::Error,
    },

    /// The record could not be serialized at all.
    #[error("evaluation record could not be serialized")]
    Serialize(#[from] serde_json::Error),

    /// A read-side failure that a summary reports rather than swallows.
    #[error("evaluation root could not be read")]
    Read {
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_carry_no_path_or_record_content() {
        let errors = [
            EvaluationError::Directory {
                source: std::io::Error::other("/secret/state/evaluations denied"),
            },
            EvaluationError::Salt {
                reason: "salt is a symlink",
                source: None,
            },
            EvaluationError::Append {
                source: std::io::Error::other("/secret/state/evaluations/x.jsonl"),
            },
            EvaluationError::Read {
                source: std::io::Error::other("/secret/state"),
            },
        ];

        for error in errors {
            let rendered = error.to_string();
            assert!(
                !rendered.contains('/'),
                "the bounded warning text must stay path-free: {rendered}"
            );
            assert!(!rendered.contains("secret"), "{rendered}");
        }
    }
}
