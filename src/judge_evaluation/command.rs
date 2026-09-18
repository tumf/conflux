//! The `cflx judge-eval summary` command body.
//!
//! Kept out of `main.rs` so the whole command — root selection, window
//! defaulting, aggregation, and both output forms — is exercisable without a
//! process, and so that "this command mutates nothing" is a property a test can
//! assert rather than a claim in a doc comment.

use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;

use super::summary::{render_human, since_from_days, summarize, SummaryQuery};
use crate::config::defaults::evaluation_root_path;

/// Fallback window when configuration offers no retention period to borrow.
const DEFAULT_SINCE_DAYS: u32 = 30;

/// Everything the summary command needs, already resolved.
///
/// Taking the root as a value rather than re-deriving it here is what lets a
/// test point the command at a fixture store without touching the real state
/// root or the environment.
#[derive(Debug, Clone)]
pub struct SummaryCommand {
    pub root: PathBuf,
    /// Judgment purpose named on the command line.
    pub purpose: String,
    pub since_days: Option<u32>,
    pub project: Option<String>,
    pub json: bool,
}

impl SummaryCommand {
    /// Resolve the command from configuration and parsed arguments.
    ///
    /// Only *storage* settings are read: required agent commands are
    /// deliberately not validated, because reading evaluation records starts
    /// nothing that would need them, exactly as `cflx logs` does not.
    ///
    /// The window defaults to the configured retention period when one is
    /// available. Summarizing a longer window than retention keeps would report
    /// a range that mostly cannot contain data.
    pub fn resolve(
        config: &crate::config::OrchestratorConfig,
        purpose: &str,
        since_days: Option<u32>,
        project: Option<String>,
        json: bool,
    ) -> Result<Self, crate::config::defaults::StateRootError> {
        let configured_retention = config
            .get_parallel_dependency_judge()
            .and_then(|judge| judge.evaluation.as_ref())
            .and_then(|evaluation| evaluation.retention_days);

        Ok(Self {
            root: evaluation_root_path(config.get_state_base_dir())?,
            purpose: purpose.to_string(),
            since_days: Some(
                since_days
                    .or(configured_retention)
                    .unwrap_or(DEFAULT_SINCE_DAYS),
            ),
            project,
            json,
        })
    }

    /// Read the store and write the summary. Returns the process exit code.
    ///
    /// Non-zero is reserved for a root-level selection or read failure. A
    /// malformed individual record is *reported* rather than escalated: it is
    /// evidence about the store, and exiting non-zero for it would make a
    /// single bad line look like a broken installation.
    pub fn run(&self, out: &mut impl Write) -> i32 {
        let query = SummaryQuery {
            purpose: self.purpose.clone(),
            since: self
                .since_days
                .map(|days| since_from_days(days, Utc::now())),
            project: self.project.clone(),
        };

        let summary = match summarize(&self.root, &query) {
            Ok(summary) => summary,
            Err(error) => {
                eprintln!("Error: {error}");
                return 1;
            }
        };

        let rendered = if self.json {
            match serde_json::to_string_pretty(&summary) {
                Ok(json) => writeln!(out, "{json}"),
                Err(error) => {
                    eprintln!("Error: {error}");
                    return 1;
                }
            }
        } else {
            render_human(&summary, out)
        };

        if let Err(error) = rendered {
            eprintln!("Error: {error}");
            return 1;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge_evaluation::record::{
        EvaluationPairRecord, EvaluationRecord, EVALUATION_SCHEMA_VERSION, OUTCOME_OBSERVED,
    };
    use crate::judge_evaluation::summary::EvaluationSummary;
    use std::fs;
    use std::path::Path;

    const PURPOSE: &str = "parallel_dependency";

    fn observed(pairs: &[(bool, bool)], duration_ms: u64) -> EvaluationRecord {
        EvaluationRecord {
            schema_version: EVALUATION_SCHEMA_VERSION,
            recorded_at: "2026-09-18T00:00:00.000Z".to_string(),
            conflux_version: "0.6.324".to_string(),
            purpose: "parallel_dependency".to_string(),
            mode: "shadow".to_string(),
            outcome: OUTCOME_OBSERVED.to_string(),
            duration_ms,
            expected_answers: pairs.len(),
            model: Some("jev-1.13.0".to_string()),
            returned_answers: Some(pairs.len()),
            input_tokens: Some(100),
            output_tokens: Some(10),
            yes_threshold: Some(0.8),
            pairs: Some(
                pairs
                    .iter()
                    .enumerate()
                    .map(|(index, (judge, analyzer))| EvaluationPairRecord {
                        pair_id: format!("sha256:{index:064x}"),
                        judge_probability: if *judge { 0.9 } else { 0.1 },
                        judge_dependency: *judge,
                        analyzer_dependency: *analyzer,
                    })
                    .collect(),
            ),
        }
    }

    /// A fixture store dated today, so the default window always includes it.
    fn fixture_store(root: &Path) {
        let dir = root.join("parallel_dependency").join("proj-abcd1234");
        fs::create_dir_all(&dir).expect("project dir");
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let lines = [
            serde_json::to_string(&observed(&[(true, true), (true, false)], 100))
                .expect("serialize"),
            serde_json::to_string(&EvaluationRecord {
                outcome: "timeout".to_string(),
                model: None,
                pairs: None,
                returned_answers: None,
                input_tokens: None,
                output_tokens: None,
                yes_threshold: None,
                duration_ms: 300,
                ..observed(&[], 300)
            })
            .expect("serialize"),
            "{ this line is not a record".to_string(),
        ];
        fs::write(dir.join(format!("{today}.jsonl")), lines.join("\n") + "\n").expect("records");
    }

    fn command(root: &Path, json: bool) -> SummaryCommand {
        SummaryCommand {
            root: root.to_path_buf(),
            purpose: "parallel_dependency".to_string(),
            since_days: Some(30),
            project: None,
            json,
        }
    }

    fn run_to_string(command: &SummaryCommand) -> (i32, String) {
        let mut out = Vec::new();
        let code = command.run(&mut out);
        (code, String::from_utf8(out).expect("output must be UTF-8"))
    }

    #[test]
    fn json_mode_emits_exactly_one_versioned_object() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        fixture_store(&root);

        let (code, output) = run_to_string(&command(&root, true));
        assert_eq!(code, 0);

        let summary: EvaluationSummary =
            serde_json::from_str(&output).expect("output must be exactly one JSON object");
        assert_eq!(
            summary.schema_version,
            crate::judge_evaluation::SUMMARY_SCHEMA_VERSION
        );
        assert_eq!(summary.purpose, "parallel_dependency");
        assert_eq!(summary.observations, 1);
        assert_eq!(summary.failures, 1);
        assert_eq!(summary.skipped_malformed, 1);
        assert_eq!(summary.confusion.true_positive, 1);
        assert_eq!(summary.confusion.false_positive, 1);
        assert_eq!(summary.precision, Some(0.5));
        assert_eq!(summary.recall, Some(1.0));
    }

    #[test]
    fn json_output_carries_no_pair_identifier() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        fixture_store(&root);

        let (_, output) = run_to_string(&command(&root, true));
        assert!(!output.contains("sha256:"), "{output}");
        assert!(!output.contains("pair_id"), "{output}");
    }

    #[test]
    fn human_mode_reports_the_same_counts_it_aggregated() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        fixture_store(&root);

        let (code, output) = run_to_string(&command(&root, false));
        assert_eq!(code, 0);

        assert!(output.contains("purpose: parallel_dependency"));
        assert!(output.contains("2 valid (1 observed, 1 failed)"));
        assert!(output.contains("1 skipped malformed"));
        assert!(output.contains("observed"));
        assert!(output.contains("timeout"));
        assert!(output.contains("TP 1 / FP 1 / FN 0 / TN 0"));
        assert!(output.contains("precision 0.500"));
        assert!(output.contains("recall 1.000"));
        assert!(output.contains("jev-1.13.0"));
        assert!(!output.contains("sha256:"));
    }

    #[test]
    fn an_undefined_rate_renders_as_not_available_rather_than_zero() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        let dir = root.join("parallel_dependency").join("proj-abcd1234");
        fs::create_dir_all(&dir).expect("dir");
        let today = Utc::now().format("%Y-%m-%d").to_string();
        fs::write(
            dir.join(format!("{today}.jsonl")),
            serde_json::to_string(&observed(&[(false, false)], 10)).expect("serialize") + "\n",
        )
        .expect("record");

        let (_, output) = run_to_string(&command(&root, false));
        assert!(
            output.contains("precision n/a") && output.contains("recall n/a"),
            "an empty denominator must not read as a failing judge: {output}"
        );
        assert!(output.contains("specificity 1.000"));
    }

    #[test]
    fn an_empty_store_succeeds_and_creates_nothing() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");

        let (code, human) = run_to_string(&command(&root, false));
        assert_eq!(code, 0, "an absent store is not an error");
        assert!(human.contains("No evaluation records matched."));
        assert!(!root.exists(), "the command must not create the root");

        let (code, json) = run_to_string(&command(&root, true));
        assert_eq!(code, 0);
        let summary: EvaluationSummary = serde_json::from_str(&json).expect("one object");
        assert_eq!(summary.valid_records(), 0);
        assert!(!root.exists());
    }

    #[test]
    fn the_command_never_mutates_the_store_it_reads() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        fixture_store(&root);

        /// Every path beneath `root` with its bytes, for exact comparison.
        fn snapshot(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
            let mut out = std::collections::BTreeMap::new();
            let mut stack = vec![root.to_path_buf()];
            while let Some(path) = stack.pop() {
                if path.is_dir() {
                    for entry in fs::read_dir(&path).expect("readable").flatten() {
                        stack.push(entry.path());
                    }
                } else {
                    out.insert(
                        path.strip_prefix(root)
                            .expect("relative")
                            .display()
                            .to_string(),
                        fs::read(&path).expect("readable"),
                    );
                }
            }
            out
        }

        let before = snapshot(&root);
        run_to_string(&command(&root, false));
        run_to_string(&command(&root, true));
        let after = snapshot(&root);

        assert_eq!(before, after, "no file may be added, removed, or rewritten");
        // Specifically: no salt was installed and no retention ran, even though
        // the fixture's only record file would be a cleanup candidate.
        assert!(!root
            .join(crate::config::defaults::EVALUATION_SALT_FILE_NAME)
            .exists());
        assert_eq!(after.len(), 1, "exactly the one fixture file remains");
    }

    #[test]
    fn the_window_defaults_to_configured_retention_then_to_thirty_days() {
        use crate::config::{
            JudgeCommandsConfig, JudgeEvaluationConfig, OrchestratorConfig,
            ParallelDependencyJudgeConfig,
        };

        let judge = |evaluation: Option<JudgeEvaluationConfig>| OrchestratorConfig {
            judge_commands: Some(JudgeCommandsConfig {
                parallel_dependency: Some(ParallelDependencyJudgeConfig {
                    command: vec!["jev".to_string()],
                    model: "jev-1.13.0".to_string(),
                    timeout_ms: None,
                    max_input_bytes: None,
                    max_output_bytes: None,
                    yes_threshold: None,
                    mode: None,
                    evaluation,
                }),
            }),
            ..Default::default()
        };

        let configured = judge(Some(JudgeEvaluationConfig {
            enabled: true,
            retention_days: Some(90),
            max_total_bytes: Some(1024),
        }));

        assert_eq!(
            SummaryCommand::resolve(&configured, PURPOSE, None, None, false)
                .expect("resolve")
                .since_days,
            Some(90),
            "the configured retention window is the natural default"
        );
        assert_eq!(
            SummaryCommand::resolve(&configured, PURPOSE, Some(7), None, false)
                .expect("resolve")
                .since_days,
            Some(7),
            "an explicit flag still wins"
        );
        assert_eq!(
            SummaryCommand::resolve(&judge(None), PURPOSE, None, None, false)
                .expect("resolve")
                .since_days,
            Some(DEFAULT_SINCE_DAYS),
            "with no policy to borrow from, the built-in default applies"
        );
        assert_eq!(
            SummaryCommand::resolve(&OrchestratorConfig::default(), PURPOSE, None, None, false)
                .expect("resolve")
                .since_days,
            Some(DEFAULT_SINCE_DAYS),
            "and with no judge configured at all"
        );
    }

    #[test]
    fn the_resolved_root_is_the_evaluation_root_never_the_log_root() {
        use crate::config::OrchestratorConfig;

        let state = tempfile::tempdir().expect("state");
        let state_base_dir = state.path().display().to_string();
        let config = OrchestratorConfig {
            state_base_dir: Some(state_base_dir.clone()),
            ..Default::default()
        };

        let resolved =
            SummaryCommand::resolve(&config, PURPOSE, None, None, false).expect("resolve");
        let log_root =
            crate::config::defaults::log_root_path(Some(&state_base_dir)).expect("log root");

        assert!(resolved.root.ends_with("evaluations"));
        assert!(!resolved.root.starts_with(&log_root));
        assert!(!log_root.starts_with(&resolved.root));
    }

    #[test]
    fn the_project_filter_selects_one_project_exactly() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        let today = Utc::now().format("%Y-%m-%d").to_string();
        for project in ["proj-aaaa1111", "proj-bbbb2222"] {
            let dir = root.join("parallel_dependency").join(project);
            fs::create_dir_all(&dir).expect("dir");
            fs::write(
                dir.join(format!("{today}.jsonl")),
                serde_json::to_string(&observed(&[(true, true)], 10)).expect("serialize") + "\n",
            )
            .expect("record");
        }

        let mut command = command(&root, true);
        command.project = Some("proj-aaaa1111".to_string());
        let (_, output) = run_to_string(&command);
        let summary: EvaluationSummary = serde_json::from_str(&output).expect("one object");
        assert_eq!(summary.files_examined, 1);
        assert_eq!(summary.project.as_deref(), Some("proj-aaaa1111"));

        command.project = None;
        let (_, output) = run_to_string(&command);
        let summary: EvaluationSummary = serde_json::from_str(&output).expect("one object");
        assert_eq!(summary.files_examined, 2);
        assert_eq!(summary.project, None);
    }
}
