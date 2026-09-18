//! Isolated, opt-in, non-authoritative judge evaluation records.
//!
//! The shadow `parallel_dependency` judge produces aggregate comparison counts
//! in the ordinary operational log. That proves an observation happened; it
//! cannot answer whether the judge is good enough to promote. Precision,
//! recall, threshold behavior, failure rate, latency, and cost all need the
//! *pair-level* probabilities and the authoritative analyzer's labels, kept
//! long enough to aggregate.
//!
//! Those records deliberately do not live in the operational log:
//!
//! - **Separation.** They are written beneath `<state-root>/evaluations`, a
//!   sibling of `logs` and never a descendant. `cflx logs`, the TUI log ring,
//!   and the Web log endpoints are scoped to `logs`, so they cannot surface an
//!   evaluation record; ordinary seven-day log retention cannot delete one; and
//!   evaluation retention here cannot delete a log.
//! - **Non-authority.** Constitutional law 1 permits external logs and metrics
//!   only as non-authoritative observability. Nothing in this module is read by
//!   analysis normalization, provenance, queue reduction, dispatch, blocker
//!   classification, Acceptance, Archive, or lifecycle routing. Deleting the
//!   whole evaluation root cannot change the next action for unchanged
//!   workspace and Git state, and every failure in here is swallowed by the
//!   caller after one bounded warning.
//! - **Privacy.** A record can name a pair only through a salted opaque digest.
//!   There is no field that could hold a change ID, proposal text, a request or
//!   response body, a command, an environment value, a repository path, a
//!   credential, or a provider error body.
//!
//! Recording is off unless `judge_commands.parallel_dependency.evaluation.enabled`
//! is explicitly true, and an enabled policy must carry both bounds: evaluation
//! storage is never unbounded.

pub mod cleanup;
pub mod command;
pub mod error;
mod fsutil;
pub mod record;
pub mod salt;
pub mod summary;
pub mod writer;

pub use cleanup::{run_cleanup, CleanupReport};
pub use command::SummaryCommand;
pub use error::{EvaluationError, EvaluationResult};
pub use record::{EvaluationRecord, EVALUATION_SCHEMA_VERSION};
pub use summary::{summarize, EvaluationSummary, SummaryQuery, SUMMARY_SCHEMA_VERSION};
pub use writer::{EvaluationPolicy, EvaluationRecorder, ObservationInput, ObservedPair};

#[cfg(test)]
mod separation_tests {
    //! Proof that evaluation storage and ordinary logs cannot reach each other.
    //!
    //! Each direction is asserted against the *real* path resolvers rather than
    //! a restatement of the layout, so a future change to either root that
    //! merged the two would fail here rather than silently exposing evaluation
    //! records through a log reader.

    use super::*;
    use crate::config::defaults::{cleanup_old_logs, get_log_file_path, log_root_path};
    use std::fs;
    use std::path::Path;

    /// A configured state root, with both stores resolved from it.
    struct Roots {
        _base: tempfile::TempDir,
        state_base_dir: String,
        log_root: std::path::PathBuf,
        evaluation_root: std::path::PathBuf,
    }

    fn roots() -> Roots {
        let base = tempfile::tempdir().expect("state base");
        let state_base_dir = base.path().display().to_string();
        Roots {
            log_root: log_root_path(Some(&state_base_dir)).expect("log root"),
            evaluation_root: crate::config::defaults::evaluation_root_path(Some(&state_base_dir))
                .expect("evaluation root"),
            state_base_dir,
            _base: base,
        }
    }

    fn write_evaluation_record(root: &Path, project: &str, date: &str, bytes: usize) {
        let dir = root.join("parallel_dependency").join(project);
        fs::create_dir_all(&dir).expect("evaluation dir");
        fs::write(dir.join(format!("{date}.jsonl")), vec![b'x'; bytes]).expect("record");
    }

    #[test]
    fn the_evaluation_root_is_a_sibling_of_the_log_root_not_a_descendant() {
        let roots = roots();

        assert_ne!(roots.evaluation_root, roots.log_root);
        assert!(
            !roots.evaluation_root.starts_with(&roots.log_root),
            "evaluation records must not live under the log root"
        );
        assert!(
            !roots.log_root.starts_with(&roots.evaluation_root),
            "logs must not live under the evaluation root"
        );
        assert_eq!(
            roots.evaluation_root.parent(),
            roots.log_root.parent(),
            "both are owned by the same state root"
        );
    }

    #[test]
    fn the_log_file_a_writer_selects_is_never_inside_the_evaluation_root() {
        let roots = roots();
        let repo_root = tempfile::tempdir().expect("repo root");

        let log_file = get_log_file_path(Some(&roots.state_base_dir), Some(repo_root.path()))
            .expect("log file path");

        assert!(log_file.starts_with(&roots.log_root));
        assert!(!log_file.starts_with(&roots.evaluation_root));
    }

    #[test]
    fn ordinary_log_retention_cannot_delete_an_evaluation_record() {
        let roots = roots();
        let repo_root = tempfile::tempdir().expect("repo root");
        let slug = crate::config::defaults::generate_project_slug(repo_root.path());

        // A stale log and a stale evaluation record, same project, same date,
        // both well past the seven-day log window.
        let log_dir = roots.log_root.join(&slug);
        fs::create_dir_all(&log_dir).expect("log dir");
        let stale_log = log_dir.join("2020-01-01.log");
        fs::write(&stale_log, b"old diagnostics").expect("log");
        write_evaluation_record(&roots.evaluation_root, &slug, "2020-01-01", 32);

        let deleted = cleanup_old_logs(Some(&roots.state_base_dir), Some(repo_root.path()), 7)
            .expect("log cleanup");

        assert_eq!(
            deleted, 1,
            "the stale log is the only thing log cleanup owns"
        );
        assert!(!stale_log.exists());
        assert!(
            roots
                .evaluation_root
                .join("parallel_dependency")
                .join(&slug)
                .join("2020-01-01.jsonl")
                .is_file(),
            "log retention must not reach an evaluation record"
        );
    }

    #[test]
    fn evaluation_retention_cannot_delete_an_ordinary_log() {
        let roots = roots();
        let slug = "proj-abcd1234";

        let log_dir = roots.log_root.join(slug);
        fs::create_dir_all(&log_dir).expect("log dir");
        let stale_log = log_dir.join("2020-01-01.log");
        fs::write(&stale_log, vec![b'x'; 4_096]).expect("log");
        // A file shaped exactly like an evaluation record, but on the log side.
        let decoy = log_dir.join("2020-01-01.jsonl");
        fs::write(&decoy, vec![b'x'; 4_096]).expect("decoy");

        write_evaluation_record(&roots.evaluation_root, slug, "2020-01-01", 32);

        let report = run_cleanup(
            &roots.evaluation_root,
            EvaluationPolicy {
                retention_days: 1,
                max_total_bytes: 1,
            },
            None,
            chrono::Utc::now().date_naive(),
        )
        .expect("evaluation cleanup");

        assert_eq!(report.considered, 1, "only its own record is inventoried");
        assert_eq!(report.removed(), 1);
        assert!(
            stale_log.is_file(),
            "evaluation cleanup must not reach logs"
        );
        assert!(decoy.is_file(), "not even a log named like a record");
    }

    #[test]
    fn cflx_logs_never_surfaces_an_evaluation_record() {
        // End-to-end through the real viewer rather than a path assertion: the
        // claim is about what an operator actually sees.
        let roots = roots();
        let repo_root = tempfile::tempdir().expect("repo root");
        let slug = crate::config::defaults::generate_project_slug(repo_root.path());
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();

        let log_dir = roots.log_root.join(&slug);
        fs::create_dir_all(&log_dir).expect("log dir");
        fs::write(
            log_dir.join(format!("{today}.log")),
            "INFO operator diagnostics line\n",
        )
        .expect("log");

        // A distinctive marker only the evaluation store contains.
        let evaluation_dir = roots
            .evaluation_root
            .join("parallel_dependency")
            .join(&slug);
        fs::create_dir_all(&evaluation_dir).expect("evaluation dir");
        fs::write(
            evaluation_dir.join(format!("{today}.jsonl")),
            "{\"marker\":\"EVALUATION-ONLY-MARKER\"}\n",
        )
        .expect("record");

        let options = crate::log_viewer::LogViewerOptions {
            print_path: false,
            last: Some(100),
            follow: false,
            today: true,
            project: Some(slug.clone()),
            repo_root: Some(repo_root.path().to_path_buf()),
            state_base_dir: Some(roots.state_base_dir.clone()),
        };

        let mut out = Vec::new();
        crate::log_viewer::run_logs_command(&options, &mut out).expect("viewer must succeed");
        let rendered = String::from_utf8(out).expect("UTF-8");

        assert!(
            rendered.contains("operator diagnostics line"),
            "the viewer must still show ordinary logs: {rendered}"
        );
        assert!(
            !rendered.contains("EVALUATION-ONLY-MARKER"),
            "no evaluation record may reach `cflx logs`: {rendered}"
        );

        // And the path it selects is on the log side of the fence.
        let selection = crate::log_viewer::resolve_log_selection(&options).expect("selection");
        assert!(selection.selected_file.starts_with(&roots.log_root));
        assert!(!selection.selected_file.starts_with(&roots.evaluation_root));
    }

    #[cfg(feature = "web-monitoring")]
    #[test]
    fn the_tui_and_web_log_ring_has_no_filesystem_source_to_leak_from() {
        use crate::web::remote_control_api::projection::Projection;

        // The ring `/api/v2/logs` and the TUI log pane read is an in-memory
        // projection of runtime events. A populated evaluation store on disk
        // cannot appear in it because the ring never reads a path at all —
        // which is what this asserts: records exist, the ring stays empty.
        let roots = roots();
        write_evaluation_record(&roots.evaluation_root, "proj-abcd1234", "2026-09-18", 256);
        assert!(roots.evaluation_root.exists(), "the store is populated");

        let projection = Projection::new();
        let (logs, _, _) = projection.logs();
        assert!(
            logs.is_empty(),
            "the log ring is fed by events, never by the evaluation root"
        );

        // Only an explicitly applied runtime entry ever appears there.
        projection.apply_log(crate::events::LogEntry::info("operator diagnostics"));
        let (logs, _, _) = projection.logs();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, "operator diagnostics");
    }

    #[test]
    fn a_disabled_policy_leaves_the_state_root_without_any_evaluation_entry() {
        use crate::config::{
            JudgeCommandsConfig, OrchestratorConfig, ParallelDependencyJudgeConfig,
        };

        let roots = roots();
        let repo_root = tempfile::tempdir().expect("repo root");
        let config = OrchestratorConfig {
            state_base_dir: Some(roots.state_base_dir.clone()),
            judge_commands: Some(JudgeCommandsConfig {
                parallel_dependency: Some(ParallelDependencyJudgeConfig {
                    command: vec!["jev".to_string()],
                    model: "jev-1.13.0".to_string(),
                    timeout_ms: None,
                    max_input_bytes: None,
                    max_output_bytes: None,
                    yes_threshold: None,
                    mode: None,
                    evaluation: None,
                }),
            }),
            ..Default::default()
        };

        assert!(EvaluationRecorder::from_config(&config, repo_root.path()).is_none());
        assert!(
            !roots.evaluation_root.exists(),
            "a disabled policy must create no directory, salt, or record"
        );
    }

    #[test]
    fn deleting_the_whole_evaluation_root_is_always_permitted_and_leaves_logs_intact() {
        // Constitutional law 1 in its operational form: the evaluation root is
        // disposable. Nothing else may depend on it existing.
        let roots = roots();
        let slug = "proj-abcd1234";
        write_evaluation_record(&roots.evaluation_root, slug, "2026-09-18", 64);
        let log_dir = roots.log_root.join(slug);
        fs::create_dir_all(&log_dir).expect("log dir");
        fs::write(log_dir.join("2026-09-18.log"), b"diagnostics").expect("log");

        fs::remove_dir_all(&roots.evaluation_root).expect("the root must be removable");

        assert!(!roots.evaluation_root.exists());
        assert!(log_dir.join("2026-09-18.log").is_file());

        // And every reader still answers, rather than failing on the absence.
        let summary = summarize(
            &roots.evaluation_root,
            &SummaryQuery {
                purpose: "parallel_dependency".to_string(),
                since: None,
                project: None,
            },
        )
        .expect("summary over a deleted root must succeed");
        assert_eq!(summary.valid_records(), 0);

        let report = run_cleanup(
            &roots.evaluation_root,
            EvaluationPolicy {
                retention_days: 30,
                max_total_bytes: 1024,
            },
            None,
            chrono::Utc::now().date_naive(),
        )
        .expect("cleanup over a deleted root must succeed");
        assert_eq!(report.considered, 0);
        assert!(
            !roots.evaluation_root.exists(),
            "neither reader recreates it"
        );
    }
}
