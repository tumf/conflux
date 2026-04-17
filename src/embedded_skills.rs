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
const CFLX_CLEANUP_REVIEW_SKILL_MD: &str = include_str!("../skills/cflx-cleanup-review/SKILL.md");
const CFLX_ACCEPT_SKILL_MD: &str = include_str!("../skills/cflx-accept/SKILL.md");
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
        assert_eq!(skills.len(), 10, "Expected exactly 10 embedded skills");
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

        // Operation-specific skills without auxiliary files: no scripts/cflx.py
        for name in &[
            "cflx-analyze",
            "cflx-rejecting",
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
}
