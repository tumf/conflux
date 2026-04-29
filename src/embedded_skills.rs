use agent_skills_rs::embedded::register_embedded_skill;
use agent_skills_rs::types::Skill;
use anyhow::Result;

// cflx-proposal skill files
const CFLX_PROPOSAL_SKILL_MD: &str = include_str!("../skills/cflx-proposal/SKILL.md");

// cflx-workflow skill files (compatibility router)
const CFLX_WORKFLOW_SKILL_MD: &str = include_str!("../skills/cflx-workflow/SKILL.md");
const CFLX_WORKFLOW_REF_ACCEPT: &str =
    include_str!("../skills/cflx-workflow/references/cflx-accept.md");
const CFLX_WORKFLOW_REF_APPLY: &str =
    include_str!("../skills/cflx-workflow/references/cflx-apply.md");
const CFLX_WORKFLOW_REF_ARCHIVE: &str =
    include_str!("../skills/cflx-workflow/references/cflx-archive.md");

// cflx-run skill files
const CFLX_RUN_SKILL_MD: &str = include_str!("../skills/cflx-run/SKILL.md");
const CFLX_RUN_REF: &str = include_str!("../skills/cflx-run/references/cflx-run.md");

// Operation-specific skill files
const CFLX_ANALYZE_SKILL_MD: &str = include_str!("../skills/cflx-analyze/SKILL.md");
const CFLX_APPLY_SKILL_MD: &str = include_str!("../skills/cflx-apply/SKILL.md");
const CFLX_APPLY_REF: &str = include_str!("../skills/cflx-apply/references/cflx-apply.md");
const CFLX_REJECTING_SKILL_MD: &str = include_str!("../skills/cflx-rejecting/SKILL.md");
const CFLX_REJECTION_GUIDE_SKILL_MD: &str = include_str!("../skills/cflx-rejection-guide/SKILL.md");
const CFLX_REJECTION_GUIDE_REF: &str =
    include_str!("../skills/cflx-rejection-guide/references/guide.md");
const CFLX_CLEANUP_REVIEW_SKILL_MD: &str = include_str!("../skills/cflx-cleanup-review/SKILL.md");
const CFLX_ACCEPT_SKILL_MD: &str = include_str!("../skills/cflx-accept/SKILL.md");
#[cfg(test)]
const CFLX_ACCEPT_COMMAND_MD: &str = include_str!("../.opencode/commands/cflx-accept.md");
const CFLX_ARCHIVE_SKILL_MD: &str = include_str!("../skills/cflx-archive/SKILL.md");
const CFLX_ARCHIVE_REF: &str = include_str!("../skills/cflx-archive/references/cflx-archive.md");
const CFLX_RESOLVE_SKILL_MD: &str = include_str!("../skills/cflx-resolve/SKILL.md");

/// Return all cflx bundled skills with their auxiliary files embedded at compile time.
pub fn get_cflx_embedded_skills() -> Result<Vec<Skill>> {
    let proposal = register_embedded_skill(CFLX_PROPOSAL_SKILL_MD, &[])?;

    let workflow = register_embedded_skill(
        CFLX_WORKFLOW_SKILL_MD,
        &[
            ("references/cflx-accept.md", CFLX_WORKFLOW_REF_ACCEPT),
            ("references/cflx-apply.md", CFLX_WORKFLOW_REF_APPLY),
            ("references/cflx-archive.md", CFLX_WORKFLOW_REF_ARCHIVE),
        ],
    )?;

    let run = register_embedded_skill(
        CFLX_RUN_SKILL_MD,
        &[("references/cflx-run.md", CFLX_RUN_REF)],
    )?;

    let analyze = register_embedded_skill(CFLX_ANALYZE_SKILL_MD, &[])?;

    let apply = register_embedded_skill(
        CFLX_APPLY_SKILL_MD,
        &[("references/cflx-apply.md", CFLX_APPLY_REF)],
    )?;

    let rejecting = register_embedded_skill(CFLX_REJECTING_SKILL_MD, &[])?;
    let rejection_guide = register_embedded_skill(
        CFLX_REJECTION_GUIDE_SKILL_MD,
        &[("references/guide.md", CFLX_REJECTION_GUIDE_REF)],
    )?;

    let cleanup_review = register_embedded_skill(CFLX_CLEANUP_REVIEW_SKILL_MD, &[])?;

    let accept = register_embedded_skill(CFLX_ACCEPT_SKILL_MD, &[])?;

    let archive = register_embedded_skill(
        CFLX_ARCHIVE_SKILL_MD,
        &[("references/cflx-archive.md", CFLX_ARCHIVE_REF)],
    )?;

    let resolve = register_embedded_skill(CFLX_RESOLVE_SKILL_MD, &[])?;

    Ok(vec![
        proposal,
        workflow,
        run,
        analyze,
        apply,
        rejecting,
        rejection_guide,
        cleanup_review,
        accept,
        archive,
        resolve,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_skills_count() {
        let skills = get_cflx_embedded_skills().expect("Failed to get embedded skills");
        assert_eq!(skills.len(), 11, "Expected exactly 11 embedded skills");
    }

    #[test]
    fn test_embedded_skills_names() {
        let skills = get_cflx_embedded_skills().unwrap();
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        let expected = [
            "cflx-proposal",
            "cflx-workflow",
            "cflx-run",
            "cflx-analyze",
            "cflx-apply",
            "cflx-rejecting",
            "cflx-rejection-guide",
            "cflx-cleanup-review",
            "cflx-accept",
            "cflx-archive",
            "cflx-resolve",
        ];
        for name in &expected {
            assert!(
                names.contains(name),
                "Expected {} skill, found: {:?}",
                name,
                names
            );
        }
    }

    #[test]
    fn test_embedded_skills_have_auxiliary_files() {
        let skills = get_cflx_embedded_skills().unwrap();

        // cflx-proposal: no scripts/cflx.py
        let proposal = skills.iter().find(|s| s.name == "cflx-proposal").unwrap();
        assert!(
            !proposal.auxiliary_files.contains_key("scripts/cflx.py"),
            "cflx-proposal must NOT have scripts/cflx.py (replaced by native CLI)"
        );

        // cflx-workflow: compatibility router with references, no scripts/cflx.py
        let workflow = skills.iter().find(|s| s.name == "cflx-workflow").unwrap();
        assert!(
            !workflow.auxiliary_files.contains_key("scripts/cflx.py"),
            "cflx-workflow must NOT have scripts/cflx.py (replaced by native CLI)"
        );
        assert!(
            workflow
                .auxiliary_files
                .contains_key("references/cflx-accept.md"),
            "cflx-workflow must have references/cflx-accept.md"
        );
        assert!(
            workflow
                .auxiliary_files
                .contains_key("references/cflx-apply.md"),
            "cflx-workflow must have references/cflx-apply.md"
        );
        assert!(
            workflow
                .auxiliary_files
                .contains_key("references/cflx-archive.md"),
            "cflx-workflow must have references/cflx-archive.md"
        );

        // cflx-run: has reference
        let run = skills.iter().find(|s| s.name == "cflx-run").unwrap();
        assert!(
            run.auxiliary_files.contains_key("references/cflx-run.md"),
            "cflx-run must have references/cflx-run.md"
        );

        // cflx-apply: has reference
        let apply = skills.iter().find(|s| s.name == "cflx-apply").unwrap();
        assert!(
            apply
                .auxiliary_files
                .contains_key("references/cflx-apply.md"),
            "cflx-apply must have references/cflx-apply.md"
        );
        assert!(
            !apply.auxiliary_files.contains_key("scripts/cflx.py"),
            "cflx-apply must NOT have scripts/cflx.py"
        );

        // cflx-archive: has reference
        let archive = skills.iter().find(|s| s.name == "cflx-archive").unwrap();
        assert!(
            archive
                .auxiliary_files
                .contains_key("references/cflx-archive.md"),
            "cflx-archive must have references/cflx-archive.md"
        );
        assert!(
            !archive.auxiliary_files.contains_key("scripts/cflx.py"),
            "cflx-archive must NOT have scripts/cflx.py"
        );

        let rejection_guide = skills
            .iter()
            .find(|s| s.name == "cflx-rejection-guide")
            .unwrap();
        assert!(
            rejection_guide
                .auxiliary_files
                .contains_key("references/guide.md"),
            "cflx-rejection-guide must have references/guide.md"
        );

        // Operation-specific skills without auxiliary files: no scripts/cflx.py
        for name in &[
            "cflx-analyze",
            "cflx-rejecting",
            "cflx-rejection-guide",
            "cflx-cleanup-review",
            "cflx-accept",
            "cflx-resolve",
        ] {
            let skill = skills.iter().find(|s| s.name == *name).unwrap();
            assert!(
                !skill.auxiliary_files.contains_key("scripts/cflx.py"),
                "{} must NOT have scripts/cflx.py",
                name
            );
        }
    }

    // --- Prompt ownership drift-detection tests ---
    // These tests verify that the workflow-split ownership boundaries are maintained:
    // - cflx-accept skill must not duplicate the fixed acceptance procedure
    // - Rust prompt builders must not contain fixed guidance phrases
    // - Embedded skills must maintain their documented role boundaries

    /// Phrases that belong exclusively to the command template (.opencode/commands/cflx-accept.md).
    /// If cflx-accept SKILL.md contains these, it means fixed procedure has drifted into the skill.
    const ACCEPTANCE_FIXED_PROCEDURE_PHRASES: &[&str] = &[
        "CRITICAL formatting rule:",
        "Forbidden wrappings",
        "NO markdown headings",
        "CRITICAL - When outputting FAIL:",
        "## Acceptance #",
    ];

    #[test]
    fn test_cflx_accept_skill_does_not_duplicate_fixed_procedure() {
        // The cflx-accept skill provides operation identity and scoped guidance only.
        // The fixed acceptance procedure (checklist, verdict formatting rules, FAIL workflow)
        // must remain in .opencode/commands/cflx-accept.md.
        for phrase in ACCEPTANCE_FIXED_PROCEDURE_PHRASES {
            assert!(
                !CFLX_ACCEPT_SKILL_MD.contains(phrase),
                "cflx-accept SKILL.md must NOT contain fixed procedure phrase '{}' — \
                 this belongs in .opencode/commands/cflx-accept.md",
                phrase
            );
        }
    }

    #[test]
    fn test_cflx_accept_skill_references_command_template() {
        // The cflx-accept skill must reference the command template as the single source
        assert!(
            CFLX_ACCEPT_SKILL_MD.contains(".opencode/commands/cflx-accept.md"),
            "cflx-accept SKILL.md must reference the command template as single source"
        );
    }

    #[test]
    fn test_acceptance_command_template_enforces_behavior_task_adequacy_in_acceptance() {
        assert!(
            CFLX_ACCEPT_COMMAND_MD
                .contains("Behavior-task adequacy check for behavior-changing work"),
            "acceptance command template must require behavior-task adequacy review in acceptance"
        );
        assert!(
            CFLX_ACCEPT_COMMAND_MD.contains(
                "Do NOT defer this class of issue to archive; acceptance owns this judgment."
            ),
            "acceptance command template must keep behavior-task adequacy ownership in acceptance"
        );
    }

    #[test]
    fn test_rust_prompt_builder_does_not_contain_acceptance_checklist() {
        // The ARCHIVE_READINESS_CONTEXT is the only fixed content the Rust prompt builder
        // injects. Verify it doesn't contain acceptance checklist items that belong
        // to the command template.
        let archive_ctx = crate::agent::prompt::tests::get_archive_readiness_context();
        let checklist_phrases = [
            "Output format (output exactly ONCE",
            "CRITICAL formatting rule:",
            "Required checks:",
            "Implementation Blocker review:",
        ];
        for phrase in &checklist_phrases {
            assert!(
                !archive_ctx.contains(phrase),
                "ARCHIVE_READINESS_CONTEXT must NOT contain acceptance checklist phrase '{}' — \
                 this belongs in .opencode/commands/cflx-accept.md",
                phrase
            );
        }
    }

    #[test]
    fn test_acceptance_verdict_contract_consistency() {
        // Verify canonical verdict markers are documented consistently
        // across the parser, the command template reference, and the cflx-workflow skill.
        let canonical_markers = [
            "ACCEPTANCE: PASS",
            "ACCEPTANCE: FAIL",
            "ACCEPTANCE: CONTINUE",
            "ACCEPTANCE: GATED",
        ];

        // cflx-workflow SKILL.md must mention all canonical markers
        for marker in &canonical_markers {
            assert!(
                CFLX_WORKFLOW_SKILL_MD.contains(marker),
                "cflx-workflow SKILL.md must document verdict marker '{}'",
                marker
            );
        }

        // cflx-workflow references/cflx-accept.md must mention all canonical markers
        for marker in &canonical_markers {
            assert!(
                CFLX_WORKFLOW_REF_ACCEPT.contains(marker),
                "cflx-workflow references/cflx-accept.md must document verdict marker '{}'",
                marker
            );
        }

        // Legacy compatibility marker should remain documented during migration.
        assert!(
            CFLX_WORKFLOW_REF_ACCEPT.contains("ACCEPTANCE: BLOCKED"),
            "cflx-workflow references/cflx-accept.md must document legacy acceptance marker 'ACCEPTANCE: BLOCKED'"
        );
    }

    #[test]
    fn test_cflx_workflow_verdict_format_prohibitions() {
        // Verify the cflx-workflow skill includes formatting prohibitions
        // to prevent drift in legacy-path output.
        assert!(
            CFLX_WORKFLOW_SKILL_MD.contains("Do NOT wrap the verdict"),
            "cflx-workflow SKILL.md must include verdict formatting prohibition"
        );
    }

    #[test]
    fn test_cflx_workflow_reference_documents_strict_canonical_contract() {
        // The runtime parser enforces standalone-line canonical matching and
        // rejects trailing-text/heading-concatenation verdicts. The embedded
        // workflow reference MUST document both the trailing-text rejection
        // example and the heading-concatenation rejection example so agents
        // are warned about the failure modes that drove this contract.
        for required in &[
            "NO trailing text",
            "PASSAll",
            "NO heading concatenation",
            "PASS## Acceptance Review Summary",
        ] {
            assert!(
                CFLX_WORKFLOW_REF_ACCEPT.contains(required),
                "cflx-workflow references/cflx-accept.md must document '{}'",
                required
            );
        }
    }
}
