use agent_skills_rs::embedded::register_embedded_skill;
use agent_skills_rs::types::Skill;
use anyhow::Result;

// cflx-proposal skill files
const CFLX_PROPOSAL_SKILL_MD: &str =
    include_str!("../skills/cflx-proposal/SKILL.md");
const CFLX_PROPOSAL_SCRIPT: &str =
    include_str!("../skills/cflx-proposal/scripts/cflx.py");

// cflx-workflow skill files
const CFLX_WORKFLOW_SKILL_MD: &str =
    include_str!("../skills/cflx-workflow/SKILL.md");
const CFLX_WORKFLOW_SCRIPT: &str =
    include_str!("../skills/cflx-workflow/scripts/cflx.py");
const CFLX_WORKFLOW_REF_ACCEPT: &str =
    include_str!("../skills/cflx-workflow/references/cflx-accept.md");
const CFLX_WORKFLOW_REF_APPLY: &str =
    include_str!("../skills/cflx-workflow/references/cflx-apply.md");
const CFLX_WORKFLOW_REF_ARCHIVE: &str =
    include_str!("../skills/cflx-workflow/references/cflx-archive.md");

// cflx-run skill files
const CFLX_RUN_SKILL_MD: &str =
    include_str!("../skills/cflx-run/SKILL.md");
const CFLX_RUN_REF: &str =
    include_str!("../skills/cflx-run/references/cflx-run.md");

/// Return all cflx bundled skills with their auxiliary files embedded at compile time.
pub fn get_cflx_embedded_skills() -> Result<Vec<Skill>> {
    let proposal = register_embedded_skill(
        CFLX_PROPOSAL_SKILL_MD,
        &[("scripts/cflx.py", CFLX_PROPOSAL_SCRIPT)],
    )?;

    let workflow = register_embedded_skill(
        CFLX_WORKFLOW_SKILL_MD,
        &[
            ("scripts/cflx.py", CFLX_WORKFLOW_SCRIPT),
            ("references/cflx-accept.md", CFLX_WORKFLOW_REF_ACCEPT),
            ("references/cflx-apply.md", CFLX_WORKFLOW_REF_APPLY),
            ("references/cflx-archive.md", CFLX_WORKFLOW_REF_ARCHIVE),
        ],
    )?;

    let run = register_embedded_skill(
        CFLX_RUN_SKILL_MD,
        &[("references/cflx-run.md", CFLX_RUN_REF)],
    )?;

    Ok(vec![proposal, workflow, run])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_skills_count() {
        let skills = get_cflx_embedded_skills().expect("Failed to get embedded skills");
        assert_eq!(skills.len(), 3, "Expected exactly 3 embedded skills");
    }

    #[test]
    fn test_embedded_skills_names() {
        let skills = get_cflx_embedded_skills().unwrap();
        let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"cflx-proposal"), "Expected cflx-proposal skill");
        assert!(names.contains(&"cflx-workflow"), "Expected cflx-workflow skill");
        assert!(names.contains(&"cflx-run"), "Expected cflx-run skill");
    }

    #[test]
    fn test_embedded_skills_have_auxiliary_files() {
        let skills = get_cflx_embedded_skills().unwrap();

        let proposal = skills.iter().find(|s| s.name == "cflx-proposal").unwrap();
        assert!(
            proposal.auxiliary_files.contains_key("scripts/cflx.py"),
            "cflx-proposal must have scripts/cflx.py"
        );

        let workflow = skills.iter().find(|s| s.name == "cflx-workflow").unwrap();
        assert!(
            workflow.auxiliary_files.contains_key("scripts/cflx.py"),
            "cflx-workflow must have scripts/cflx.py"
        );
        assert!(
            workflow.auxiliary_files.contains_key("references/cflx-accept.md"),
            "cflx-workflow must have references/cflx-accept.md"
        );
        assert!(
            workflow.auxiliary_files.contains_key("references/cflx-apply.md"),
            "cflx-workflow must have references/cflx-apply.md"
        );
        assert!(
            workflow.auxiliary_files.contains_key("references/cflx-archive.md"),
            "cflx-workflow must have references/cflx-archive.md"
        );

        let run = skills.iter().find(|s| s.name == "cflx-run").unwrap();
        assert!(
            run.auxiliary_files.contains_key("references/cflx-run.md"),
            "cflx-run must have references/cflx-run.md"
        );
    }
}
