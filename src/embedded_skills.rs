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
const CFLX_ACCEPT_WITH_SPECA_SKILL_MD: &str =
    include_str!("../skills/cflx-accept-with-speca/SKILL.md");
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
    let accept_with_speca = register_embedded_skill(CFLX_ACCEPT_WITH_SPECA_SKILL_MD, &[])?;

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
        accept_with_speca,
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
        assert_eq!(skills.len(), 12, "Expected exactly 12 embedded skills");
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
            "cflx-accept-with-speca",
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
            "cflx-accept-with-speca",
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
    // - cflx-accept skill owns the portable acceptance operation interface
    // - Rust prompt builders inject variable context rather than provider-specific checklists
    // - Embedded skills maintain their documented role boundaries

    /// Phrases that indicate provider-specific command-template coupling or
    /// apply-loop mutation workflow details that must not become the portable skill interface.
    const ACCEPTANCE_PORTABILITY_FORBIDDEN_PHRASES: &[&str] = &[
        "single source of truth",
        "command template as authoritative",
        "command-template contract",
        "opencode",
        "OpenCode",
        "agent-exec run --",
        "On mini",
        "## Acceptance #",
    ];

    #[test]
    fn test_verification_phase_guidance_is_embedded() {
        for required in [
            "pre-integration",
            "post-integration",
            "repository-automation",
        ] {
            assert!(CFLX_PROPOSAL_SKILL_MD.contains(required));
            assert!(CFLX_ACCEPT_SKILL_MD.contains(required));
            assert!(CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(required));
        }
        for required in [
            "external HTTP/API checks",
            "id`, `requirement`, `phase`, `owner`, `trigger`, `automation`, `evidence`, `rerun`, and `prerequisites`",
        ] {
            assert!(CFLX_PROPOSAL_SKILL_MD.contains(required));
        }
        for required in [
            "undeployed or external target",
            "repository-fixable FAIL",
            "is not a FAIL",
            "stalled hold",
            "unobserved",
        ] {
            assert!(CFLX_ACCEPT_SKILL_MD.contains(required));
            assert!(CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(required));
        }
    }

    #[test]
    fn test_cflx_accept_skill_defines_portable_contract() {
        for required in &[
            "portable Conflux acceptance interface",
            "agent-runtime independent",
            "Runtime-specific entrypoints are adapters",
            "{\"acceptance\":\"pass\"}",
            "{\"acceptance\":\"fail\",\"findings\":[\"<evidence>\"]}",
            "{\"acceptance\":\"continue\"}",
            "{\"acceptance\":\"gated\"}",
            "ACCEPTANCE: PASS",
            "ACCEPTANCE: FAIL",
            "ACCEPTANCE: CONTINUE",
            "ACCEPTANCE: GATED",
        ] {
            assert!(
                CFLX_ACCEPT_SKILL_MD.contains(required),
                "cflx-accept SKILL.md must define portable acceptance contract: {required}"
            );
        }
        assert!(
            !CFLX_ACCEPT_SKILL_MD.contains(".opencode/commands/cflx-accept.md"),
            "cflx-accept SKILL.md must not require runtime-specific command files for its interface"
        );
        for phrase in ACCEPTANCE_PORTABILITY_FORBIDDEN_PHRASES {
            assert!(
                !CFLX_ACCEPT_SKILL_MD.contains(phrase),
                "cflx-accept SKILL.md must not contain provider-coupled phrase '{}'",
                phrase
            );
        }
    }

    #[test]
    fn test_cflx_accept_with_speca_skill_contract() {
        let skills = get_cflx_embedded_skills().unwrap();
        let skill = skills
            .iter()
            .find(|s| s.name == "cflx-accept-with-speca")
            .expect("cflx-accept-with-speca must be embedded");

        assert_eq!(skill.name, "cflx-accept-with-speca");
        for required in &[
            "drop-in replacement for `cflx-accept`",
            "exact same verdict output interface as `cflx-accept`",
            "agent-runtime independent",
            "Runtime-specific entrypoints are adapters",
            "{\"acceptance\":\"pass\"}",
            "{\"acceptance\":\"fail\",\"findings\":[\"<evidence>\"]}",
            "{\"acceptance\":\"continue\"}",
            "{\"acceptance\":\"gated\"}",
            "ACCEPTANCE: PASS",
            "ACCEPTANCE: FAIL",
            "ACCEPTANCE: CONTINUE",
            "ACCEPTANCE: GATED",
        ] {
            assert!(
                CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(required),
                "SPECA acceptance skill must preserve portable cflx-accept interface: {required}"
            );
        }
        assert!(
            !CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(".opencode/commands/cflx-accept.md"),
            "SPECA acceptance skill must not require runtime-specific command files for its interface"
        );
        assert!(
            CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("Derive checkable properties")
                && CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("Attempt proof or falsification"),
            "SPECA acceptance skill must include property derivation and proof-attempt guidance"
        );
        assert!(
            CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("standard JSON `fail` verdict")
                && CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("findings"),
            "SPECA acceptance skill must map blocking property failures to standard fail findings"
        );
        assert!(
            CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("unavailable SPECA tooling")
                && CFLX_ACCEPT_WITH_SPECA_SKILL_MD
                    .contains("Never treat unavailable SPECA tooling as an automatic pass"),
            "SPECA acceptance skill must define fallback behavior when tooling is unavailable"
        );
        assert!(
            CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("Do not ask the user questions"),
            "SPECA acceptance skill must remain autonomous"
        );
        assert!(
            CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("out-of-worktree durable"),
            "SPECA acceptance skill must preserve workspace-local workflow-control constraints"
        );
    }

    #[test]
    fn test_cflx_accept_with_speca_documents_official_runner_adapter() {
        assert!(
            CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("NyxFoundation/speca")
                && CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("$SPECA_CHECKOUT")
                && CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("~/services/speca/")
                && CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(
                    "~/tmp/cflx-speca/<workspace-key>/run/<change-id>/<attempt-id>/speca/"
                )
                && CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("uv run python3 scripts/run_phase.py"),
            "SPECA acceptance skill must document the official runner checkout and command shape"
        );
        assert!(
            CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("Official NyxFoundation/speca Runner Adapter"),
            "SPECA acceptance skill must include a dedicated official runner adapter section"
        );
    }

    #[test]
    fn test_cflx_accept_with_speca_keeps_runner_artifacts_outside_worktree() {
        for required in &[
            "~/tmp/cflx-speca/<workspace-key>/",
            "~/tmp/cflx-speca/<workspace-key>/input/<change-id>/<attempt-id>/",
            "~/tmp/cflx-speca/<workspace-key>/output/<change-id>/<attempt-id>/",
            "outside the target Conflux worktree",
            "Do not clone or install NyxFoundation/speca from this acceptance skill",
            "Deleting the out-of-worktree SPECA input/output/log/cache directories must not change the next Conflux action",
            "repository/workspace evidence is authoritative",
        ] {
            assert!(
                CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(required),
                "SPECA acceptance skill must contain workspace-boundary guidance: {required}"
            );
        }
        assert!(
            !CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains("~/tmp/speca-conflux-input/<change-id>/")
                && !CFLX_ACCEPT_WITH_SPECA_SKILL_MD
                    .contains("~/tmp/speca-conflux-output/<change-id>/"),
            "SPECA acceptance skill must not recommend global change-id-only input/output paths"
        );
    }

    #[test]
    fn test_cflx_accept_with_speca_documents_prerequisites_and_fallback() {
        for required in &[
            "`uv` is installed and available on `PATH`",
            "Python dependencies are ready",
            "Required model/API/session/auth access is available",
            "record the limitation in human-readable reasoning and continue with manual SPECA-style property review",
            "Never treat runner unavailability, setup failure, missing auth, or inconclusive output as an automatic pass",
        ] {
            assert!(
                CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(required),
                "SPECA acceptance skill must contain prerequisite/fallback guidance: {required}"
            );
        }
    }

    #[test]
    fn test_cflx_accept_with_speca_documents_runtime_neutral_long_runner_work() {
        for required in &[
            "Use the current runtime's standard long-running-command mechanism",
            "uv sync",
            "uv run python3 scripts/run_phase.py",
        ] {
            assert!(
                CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(required),
                "SPECA runner setup/execution guidance must remain runtime-neutral: {required}"
            );
        }
        for forbidden in &["agent-exec run --", "On mini", "opencode", "OpenCode"] {
            assert!(
                !CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(forbidden),
                "SPECA runner guidance must not depend on a specific harness: {forbidden}"
            );
        }
    }

    #[test]
    fn test_cflx_accept_with_speca_skill_remains_portable() {
        for phrase in ACCEPTANCE_PORTABILITY_FORBIDDEN_PHRASES {
            assert!(
                !CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(phrase),
                "cflx-accept-with-speca SKILL.md must not contain provider-coupled phrase '{}'",
                phrase
            );
        }
    }

    #[test]
    fn test_cflx_accept_with_speca_skill_has_no_speca_terminal_protocol() {
        for forbidden in &[
            "SPECA: PASS",
            "SPECA: FAIL",
            "SPECA: CONTINUE",
            "SPECA: GATED",
        ] {
            assert!(
                !CFLX_ACCEPT_WITH_SPECA_SKILL_MD.contains(forbidden),
                "cflx-accept-with-speca must not introduce terminal protocol '{}'",
                forbidden
            );
        }
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
        // injects. Verify it doesn't contain the portable acceptance checklist
        // owned by the acceptance operation skill or runtime-specific adapters.
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
                "ARCHIVE_READINESS_CONTEXT must NOT contain acceptance checklist phrase '{}'",
                phrase
            );
        }
    }

    #[test]
    fn test_rejecting_skill_documents_block_tri_state_contract() {
        for required in &[
            "REJECTION_REVIEW: CONFIRM",
            "REJECTION_REVIEW: RESUME",
            "REJECTION_REVIEW: BLOCK",
            "CONFIRM`: terminal rejection",
            "RESUME`: reject proposal is dismissed",
            "BLOCK`: reject proposal describes a real non-terminal blocker",
        ] {
            assert!(
                CFLX_REJECTING_SKILL_MD.contains(required),
                "cflx-rejecting SKILL.md must document tri-state rejecting outcome: {required}"
            );
        }
        assert!(
            !CFLX_REJECTING_SKILL_MD.contains("CONFIRM|RESUME")
                && !CFLX_REJECTING_SKILL_MD.contains("CONFIRM or RESUME"),
            "cflx-rejecting must not describe rejecting review as confirm/resume-only"
        );
    }

    #[test]
    fn apply_skill_preserves_runtime_owned_acceptance_follow_up() {
        for (label, content) in &[
            ("cflx-apply", CFLX_APPLY_SKILL_MD),
            ("cflx-apply reference", CFLX_APPLY_REF),
        ] {
            assert!(
                content.contains("runtime-owned acceptance follow-up"),
                "{label} must identify acceptance follow-up ownership"
            );
            assert!(
                content.contains("Do not delete or move"),
                "{label} must preserve unresolved acceptance findings"
            );
            assert!(
                content.contains("mark each existing finding `[x]`")
                    || content.contains("mark each existing finding `- [x]`"),
                "{label} must close findings only after fixing and verifying them"
            );
        }
    }

    #[test]
    fn test_apply_and_acceptance_skills_keep_recoverable_infra_blockers_non_terminal() {
        for (label, content) in &[
            ("cflx-apply", CFLX_APPLY_SKILL_MD),
            ("cflx-accept", CFLX_ACCEPT_SKILL_MD),
            ("cflx-workflow", CFLX_WORKFLOW_SKILL_MD),
        ] {
            assert!(
                content.contains("infrastructure")
                    && content.contains("non-terminal")
                    && content.contains("stalled"),
                "{label} must route recoverable infrastructure blockers to non-terminal stalled holds"
            );
            assert!(
                content.contains("Docker")
                    && content.contains("credential")
                    && content.contains("pending"),
                "{label} must mention representative recoverable blocker classes"
            );
        }
        assert!(
            CFLX_APPLY_SKILL_MD.contains("do not create `REJECTED.md` for these recoverable cases"),
            "cflx-apply must prohibit REJECTED.md for recoverable infrastructure blockers"
        );
    }

    #[test]
    fn test_acceptance_skills_frame_blockers_as_stalled_holds() {
        for (label, content) in &[
            ("cflx-accept", CFLX_ACCEPT_SKILL_MD),
            ("cflx-accept-with-speca", CFLX_ACCEPT_WITH_SPECA_SKILL_MD),
            ("cflx-workflow", CFLX_WORKFLOW_SKILL_MD),
            (
                "cflx-workflow references/cflx-accept.md",
                CFLX_WORKFLOW_REF_ACCEPT,
            ),
            (".opencode/commands/cflx-accept.md", CFLX_ACCEPT_COMMAND_MD),
        ] {
            assert!(
                content.contains("stalled") && content.contains("Implementation Blocker"),
                "{label} must describe valid Implementation Blockers as stalled holds"
            );
            assert!(
                content.contains("compatibility") && content.contains("{\"acceptance\":\"gated\"}"),
                "{label} must keep gated as a parser-compatible handoff token"
            );
            assert!(
                !content.contains("- GATED:") && !content.contains("FAIL vs GATED"),
                "{label} must not use GATED as the primary blocker outcome label or rubric"
            );
            assert!(
                !content.contains("STALLED HOLD (primary): {\"acceptance\":\"stalled\"}")
                    && !content.contains("- STALLED:    {\"acceptance\":\"stalled\"}"),
                "{label} must not define a stalled JSON verdict as a supported output before parser support exists"
            );
        }
    }

    #[test]
    fn test_acceptance_verdict_contract_consistency() {
        // Verify canonical verdict markers are documented consistently
        // across the parser, OpenCode adapter reference, and the cflx-workflow skill.
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
