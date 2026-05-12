use std::fmt;

/// Stable identifier for one in-memory orchestrator instance.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OrchestratorId(String);

/// Stable identifier for one Conflux project.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectId(String);

/// Stable identifier for one OpenSpec proposal/change.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProposalId(String);

macro_rules! id_type {
    ($type_name:ident) => {
        impl $type_name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl From<&str> for $type_name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $type_name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $type_name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

id_type!(OrchestratorId);
id_type!(ProjectId);
id_type!(ProposalId);

impl Default for OrchestratorId {
    fn default() -> Self {
        Self::new("orchestrator")
    }
}

impl Default for ProjectId {
    fn default() -> Self {
        Self::new("default")
    }
}

impl ProposalId {
    /// Compatibility bridge from an existing OpenSpec change ID to the new proposal ID.
    pub fn from_change_id(change_id: impl Into<String>) -> Self {
        Self::new(change_id)
    }

    /// Compatibility bridge back to existing change-oriented APIs.
    pub fn as_change_id(&self) -> &str {
        self.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_round_trip_as_stable_strings() {
        let orchestrator = OrchestratorId::new("orch-a");
        let project = ProjectId::new("project-a");
        let proposal = ProposalId::new("change-a");

        assert_eq!(orchestrator.to_string(), "orch-a");
        assert_eq!(project.to_string(), "project-a");
        assert_eq!(proposal.to_string(), "change-a");
        assert_eq!(ProposalId::from_change_id("change-a").as_change_id(), "change-a");
    }
}
