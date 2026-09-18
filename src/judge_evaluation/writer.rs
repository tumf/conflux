//! The evaluation recorder: root resolution, salt, and concurrency-safe append.
//!
//! The recorder is the only place raw change IDs meet the evaluation store, and
//! they do not survive the meeting: they are consumed to derive a salted opaque
//! pair identifier and are never held in a record, a file name, or a directory
//! name. Everything the recorder can fail at is swallowed by its caller after
//! one bounded warning, which is what keeps evaluation data non-authoritative.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};

use super::error::{EvaluationError, EvaluationResult};
use super::record::{
    EvaluationPairRecord, EvaluationRecord, EVALUATION_SCHEMA_VERSION, OUTCOME_OBSERVED,
};
use super::salt::{load_or_create_salt, PairIdSalt};
use crate::config::defaults::{
    evaluation_root_path, generate_project_slug, EVALUATION_PURPOSE_PARALLEL_DEPENDENCY,
};

/// Conflux version stamped into every record.
const CONFLUX_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Validated evaluation bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationPolicy {
    pub retention_days: u32,
    pub max_total_bytes: u64,
}

/// Clock source, injectable so a record's timestamp and daily file are
/// deterministic under test without a sleep or a wall-clock assertion.
#[derive(Debug, Clone, Copy)]
enum Clock {
    System,
    Fixed(DateTime<Utc>),
}

impl Clock {
    fn now(self) -> DateTime<Utc> {
        match self {
            Self::System => Utc::now(),
            Self::Fixed(instant) => instant,
        }
    }
}

/// One directed judgment the analyzer observed, before pseudonymization.
///
/// The change IDs here are borrowed and immediately consumed: nothing that
/// leaves this module carries them.
#[derive(Debug, Clone)]
pub struct ObservedPair<'a> {
    pub dependent: &'a str,
    pub dependency: &'a str,
    pub judge_probability: f64,
    pub analyzer_dependency: bool,
}

/// A complete valid shadow observation awaiting persistence.
#[derive(Debug, Clone)]
pub struct ObservationInput<'a> {
    pub model: &'a str,
    pub duration_ms: u64,
    pub expected_answers: usize,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub yes_threshold: f64,
    pub pairs: Vec<ObservedPair<'a>>,
}

/// Writes evaluation records for one project beneath one dedicated root.
#[derive(Debug, Clone)]
pub struct EvaluationRecorder {
    root: PathBuf,
    project_slug: String,
    /// Judge mode stamped into each record, taken from the configuration that
    /// produced this recorder rather than assumed: a record must say which mode
    /// actually produced it, so evidence collected under one mode can never be
    /// mistaken for evidence collected under another.
    mode: String,
    policy: EvaluationPolicy,
    clock: Clock,
}

impl EvaluationRecorder {
    /// Build a recorder for an explicit root, project, mode, and policy.
    pub fn new(
        root: PathBuf,
        project_slug: String,
        mode: String,
        policy: EvaluationPolicy,
    ) -> Self {
        Self {
            root,
            project_slug,
            mode,
            policy,
            clock: Clock::System,
        }
    }

    /// Build a recorder only when the configuration opts in.
    ///
    /// `None` is the ordinary case and covers every reason not to record: no
    /// judge entry, no `evaluation` object, `enabled: false`, bounds that did
    /// not validate, or a state root that cannot be resolved. A disabled
    /// configuration therefore never reaches a filesystem call at all — no
    /// directory, no salt, no record.
    pub fn from_config(
        config: &crate::config::OrchestratorConfig,
        repo_root: &Path,
    ) -> Option<Self> {
        let judge = config.get_parallel_dependency_judge()?;
        let (retention_days, max_total_bytes) = judge.evaluation_bounds()?;
        let root = evaluation_root_path(config.get_state_base_dir()).ok()?;

        Some(Self::new(
            root,
            generate_project_slug(repo_root),
            judge.mode().to_string(),
            EvaluationPolicy {
                retention_days,
                max_total_bytes,
            },
        ))
    }

    /// Pin the clock, for deterministic tests.
    #[cfg(test)]
    pub fn with_fixed_clock(mut self, instant: DateTime<Utc>) -> Self {
        self.clock = Clock::Fixed(instant);
        self
    }

    /// The dedicated evaluation root this recorder writes beneath.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Bounds this recorder's cleanup enforces.
    pub fn policy(&self) -> EvaluationPolicy {
        self.policy
    }

    /// Directory holding this project's daily files.
    pub fn project_dir(&self) -> PathBuf {
        self.root
            .join(EVALUATION_PURPOSE_PARALLEL_DEPENDENCY)
            .join(&self.project_slug)
    }

    /// The daily file a record written `now` belongs to.
    fn current_file(&self, now: DateTime<Utc>) -> PathBuf {
        self.project_dir()
            .join(format!("{}.jsonl", now.format("%Y-%m-%d")))
    }

    /// Persist one valid shadow observation.
    pub fn record_observation(&self, input: &ObservationInput<'_>) -> EvaluationResult<()> {
        let now = self.clock.now();
        let salt = self.salt()?;

        let pairs = input
            .pairs
            .iter()
            .map(|pair| EvaluationPairRecord {
                pair_id: salt.pair_id(&self.project_slug, pair.dependent, pair.dependency),
                judge_probability: pair.judge_probability,
                judge_dependency: pair.judge_probability >= input.yes_threshold,
                analyzer_dependency: pair.analyzer_dependency,
            })
            .collect::<Vec<_>>();

        self.append(
            EvaluationRecord {
                schema_version: EVALUATION_SCHEMA_VERSION,
                recorded_at: now.to_rfc3339_opts(SecondsFormat::Millis, true),
                conflux_version: CONFLUX_VERSION.to_string(),
                purpose: EVALUATION_PURPOSE_PARALLEL_DEPENDENCY.to_string(),
                mode: self.mode.clone(),
                outcome: OUTCOME_OBSERVED.to_string(),
                duration_ms: input.duration_ms,
                expected_answers: input.expected_answers,
                model: Some(input.model.to_string()),
                returned_answers: Some(pairs.len()),
                input_tokens: Some(input.input_tokens),
                output_tokens: Some(input.output_tokens),
                yes_threshold: Some(input.yes_threshold),
                pairs: Some(pairs),
            },
            now,
        )
    }

    /// Persist one bounded judge failure.
    ///
    /// Model and pairs are deliberately absent: a failure means no response
    /// passed schema validation, so there is no model identity worth trusting
    /// and no pair judgment to record. Only the typed category, the measured
    /// duration, and the question count are evidence.
    pub fn record_failure(
        &self,
        outcome: &str,
        duration_ms: u64,
        expected_answers: usize,
    ) -> EvaluationResult<()> {
        let now = self.clock.now();
        self.append(
            EvaluationRecord {
                schema_version: EVALUATION_SCHEMA_VERSION,
                recorded_at: now.to_rfc3339_opts(SecondsFormat::Millis, true),
                conflux_version: CONFLUX_VERSION.to_string(),
                purpose: EVALUATION_PURPOSE_PARALLEL_DEPENDENCY.to_string(),
                mode: self.mode.clone(),
                outcome: outcome.to_string(),
                duration_ms,
                expected_answers,
                model: None,
                returned_answers: None,
                input_tokens: None,
                output_tokens: None,
                yes_threshold: None,
                pairs: None,
            },
            now,
        )
    }

    /// Load the installation salt, creating the root and salt on first use.
    fn salt(&self) -> EvaluationResult<PairIdSalt> {
        load_or_create_salt(&self.root)
    }

    /// Append one serialized record to the current daily file.
    ///
    /// The record is serialized to one complete buffer *before* the file is
    /// opened, so a serialization failure never leaves a partial line behind,
    /// and the single `write_all` into an `O_APPEND` descriptor is what keeps
    /// concurrent writers from interleaving.
    fn append(&self, record: EvaluationRecord, now: DateTime<Utc>) -> EvaluationResult<()> {
        let line = record.to_jsonl()?;
        let path = self.current_file(now);

        super::fsutil::create_private_dir_all(path.parent().unwrap_or(&self.root))?;

        // Retention runs before the file is opened so the current writer is
        // already a known path when eviction picks victims, and once per root
        // per process so a busy analysis loop is not an I/O loop.
        self.run_cleanup_once(&path, now.date_naive());

        let mut file = super::fsutil::open_private_append(&path)?;
        file.write_all(line.as_bytes())
            .map_err(|source| EvaluationError::Append { source })?;
        file.flush()
            .map_err(|source| EvaluationError::Append { source })?;
        Ok(())
    }

    /// Run retention at most once per evaluation root per process.
    ///
    /// Failures are intentionally dropped here rather than propagated: a
    /// retention problem must not stop the record that prompted it from being
    /// written, and it must not become the analysis's problem either. The
    /// report is returned by [`super::cleanup::run_cleanup`] for callers that
    /// do want to inspect it.
    fn run_cleanup_once(&self, current: &Path, today: NaiveDate) {
        use std::collections::HashSet;
        use std::sync::{Mutex, OnceLock};

        static CLEANED_ROOTS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
        let cleaned = CLEANED_ROOTS.get_or_init(|| Mutex::new(HashSet::new()));

        {
            // A poisoned lock means another thread panicked mid-insert; the
            // worst outcome of ignoring that is one extra cleanup pass, which
            // is strictly better than refusing to record.
            let mut guard = match cleaned.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if !guard.insert(self.root.clone()) {
                return;
            }
        }

        let _ = super::cleanup::run_cleanup(&self.root, self.policy, Some(current), today);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::defaults::JUDGE_MODE_SHADOW;
    use crate::judge_evaluation::salt::salt_path;
    use std::fs;

    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-18T12:30:45.123Z")
            .expect("fixture instant")
            .with_timezone(&Utc)
    }

    fn recorder(root: &Path) -> EvaluationRecorder {
        EvaluationRecorder::new(
            root.to_path_buf(),
            "proj-abcd1234".to_string(),
            JUDGE_MODE_SHADOW.to_string(),
            EvaluationPolicy {
                retention_days: 30,
                max_total_bytes: 10 * 1024 * 1024,
            },
        )
        .with_fixed_clock(fixed_now())
    }

    fn observation<'a>() -> ObservationInput<'a> {
        ObservationInput {
            model: "jev-1.13.0",
            duration_ms: 892,
            expected_answers: 2,
            input_tokens: 1038,
            output_tokens: 56,
            yes_threshold: 0.8,
            pairs: vec![
                ObservedPair {
                    dependent: "add-secret-alpha",
                    dependency: "add-secret-beta",
                    judge_probability: 0.97,
                    analyzer_dependency: true,
                },
                ObservedPair {
                    dependent: "add-secret-beta",
                    dependency: "add-secret-alpha",
                    judge_probability: 0.12,
                    analyzer_dependency: false,
                },
            ],
        }
    }

    fn read_records(root: &Path) -> Vec<EvaluationRecord> {
        let file = root
            .join("parallel_dependency")
            .join("proj-abcd1234")
            .join("2026-09-18.jsonl");
        fs::read_to_string(file)
            .expect("daily file must exist")
            .lines()
            .map(|line| serde_json::from_str(line).expect("each line must be one record"))
            .collect()
    }

    #[test]
    fn observation_is_written_to_the_dated_project_file() {
        let base = tempfile::tempdir().expect("temp base");
        let root = base.path().join("evaluations");
        recorder(&root)
            .record_observation(&observation())
            .expect("record must be written");

        let records = read_records(&root);
        assert_eq!(records.len(), 1);
        let record = &records[0];

        assert_eq!(record.schema_version, EVALUATION_SCHEMA_VERSION);
        assert_eq!(record.recorded_at, "2026-09-18T12:30:45.123Z");
        assert_eq!(record.conflux_version, CONFLUX_VERSION);
        assert_eq!(record.purpose, "parallel_dependency");
        assert_eq!(record.mode, "shadow");
        assert_eq!(record.outcome, OUTCOME_OBSERVED);
        assert_eq!(record.duration_ms, 892);
        assert_eq!(record.expected_answers, 2);
        assert_eq!(record.model.as_deref(), Some("jev-1.13.0"));
        assert_eq!(record.returned_answers, Some(2));
        assert_eq!(record.input_tokens, Some(1038));
        assert_eq!(record.output_tokens, Some(56));
        assert_eq!(record.yes_threshold, Some(0.8));

        let pairs = record.pairs.as_ref().expect("pairs must be present");
        assert_eq!(pairs.len(), 2);
        assert!(pairs[0].judge_dependency, "0.97 >= 0.8 is a yes edge");
        assert!(pairs[0].analyzer_dependency);
        assert!(!pairs[1].judge_dependency, "0.12 < 0.8 is a no edge");
        assert!(!pairs[1].analyzer_dependency);
    }

    #[test]
    fn raw_probability_is_preserved_so_thresholds_can_be_recomputed() {
        let base = tempfile::tempdir().expect("temp base");
        let root = base.path().join("evaluations");
        recorder(&root)
            .record_observation(&observation())
            .expect("record");

        let pairs = read_records(&root)[0]
            .pairs
            .clone()
            .expect("pairs must be present");
        // The stored threshold plus the raw probabilities are exactly what an
        // alternate-threshold sweep needs; nothing is lost to the boolean.
        assert!((pairs[0].judge_probability - 0.97).abs() < f64::EPSILON);
        assert!((pairs[1].judge_probability - 0.12).abs() < f64::EPSILON);
        let at_half: Vec<bool> = pairs.iter().map(|p| p.judge_probability >= 0.5).collect();
        assert_eq!(at_half, vec![true, false]);
    }

    #[test]
    fn no_raw_change_id_reaches_the_record_bytes_or_any_path() {
        let base = tempfile::tempdir().expect("temp base");
        let root = base.path().join("evaluations");
        recorder(&root)
            .record_observation(&observation())
            .expect("record");

        let path = root
            .join("parallel_dependency")
            .join("proj-abcd1234")
            .join("2026-09-18.jsonl");
        let bytes = fs::read_to_string(&path).expect("read");
        let rendered_path = path.display().to_string();

        for secret in ["add-secret-alpha", "add-secret-beta", "secret"] {
            assert!(
                !bytes.contains(secret),
                "`{secret}` must not survive into record bytes"
            );
            assert!(
                !rendered_path.contains(secret),
                "`{secret}` must not survive into a path"
            );
        }
    }

    #[test]
    fn failure_record_carries_only_bounded_metadata() {
        let base = tempfile::tempdir().expect("temp base");
        let root = base.path().join("evaluations");
        recorder(&root)
            .record_failure("timeout", 30_000, 6)
            .expect("failure must be recorded");

        let records = read_records(&root);
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.outcome, "timeout");
        assert_eq!(record.duration_ms, 30_000);
        assert_eq!(record.expected_answers, 6);
        assert!(record.model.is_none(), "no model was ever validated");
        assert!(record.pairs.is_none(), "no pair was ever judged");
        assert!(record.returned_answers.is_none());
        assert!(record.input_tokens.is_none());
        assert!(record.yes_threshold.is_none());
    }

    #[test]
    fn appends_accumulate_without_truncating_earlier_records() {
        let base = tempfile::tempdir().expect("temp base");
        let root = base.path().join("evaluations");
        let recorder = recorder(&root);

        recorder.record_observation(&observation()).expect("first");
        recorder.record_failure("spawn", 1, 2).expect("second");
        recorder.record_observation(&observation()).expect("third");

        let records = read_records(&root);
        assert_eq!(records.len(), 3);
        assert!(records[0].is_observed());
        assert_eq!(records[1].outcome, "spawn");
        assert!(records[2].is_observed());
    }

    #[test]
    fn concurrent_writers_produce_only_complete_parseable_lines() {
        let base = tempfile::tempdir().expect("temp base");
        let root = base.path().join("evaluations");
        // Deliberately *not* seeded: the threads race on first-time root
        // creation, salt installation, and the append all at once, which is the
        // arrangement a real cold start produces.
        let seed = 1;

        const THREADS: usize = 8;
        const PER_THREAD: usize = 16;

        std::thread::scope(|scope| {
            for thread in 0..THREADS {
                let root = root.clone();
                scope.spawn(move || {
                    let recorder = recorder(&root);
                    for _ in 0..PER_THREAD {
                        recorder
                            .record_observation(&observation())
                            .expect("concurrent record");
                        recorder
                            .record_failure("transient", thread as u64, thread)
                            .expect("concurrent failure");
                    }
                });
            }
        });

        let records = read_records(&root);
        assert_eq!(
            records.len(),
            THREADS * PER_THREAD * 2,
            "every append must land exactly once as one complete line"
        );
        assert_eq!(
            records.iter().filter(|r| r.is_observed()).count(),
            THREADS * PER_THREAD
        );

        // One salt for the whole race: every observation must be comparable to
        // every other, which it is not if a loser installed a second salt.
        let pair_ids: std::collections::BTreeSet<&str> = records
            .iter()
            .filter_map(|record| record.pairs.as_ref())
            .flatten()
            .map(|pair| pair.pair_id.as_str())
            .collect();
        assert_eq!(
            pair_ids.len(),
            2,
            "the two fixture pairs must resolve to exactly two identities"
        );
        let _ = seed;
    }

    #[test]
    fn a_cold_start_race_installs_exactly_one_salt() {
        // Separated from the append test because the failure it guards is
        // specific: two threads picking the same temporary name, where the
        // loser's cleanup would destroy the winner's in-flight file and leave
        // *neither* with a usable salt.
        let base = tempfile::tempdir().expect("temp base");
        let root = base.path().join("evaluations");

        let barrier = std::sync::Barrier::new(16);
        std::thread::scope(|scope| {
            for _ in 0..16 {
                let root = root.clone();
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    crate::judge_evaluation::salt::load_or_create_salt(&root)
                        .expect("every racing caller must end up with the installed salt")
                });
            }
        });

        let entries: Vec<String> = fs::read_dir(&root)
            .expect("root")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with(".pair-id-salt"))
            .collect();
        assert_eq!(
            entries,
            vec![".pair-id-salt".to_string()],
            "exactly one salt, and no temporary left behind"
        );
    }

    #[cfg(unix)]
    #[test]
    fn record_directories_and_files_are_private() {
        use std::os::unix::fs::PermissionsExt;

        let base = tempfile::tempdir().expect("temp base");
        let root = base.path().join("evaluations");
        let recorder = recorder(&root);
        recorder.record_observation(&observation()).expect("record");

        let file = recorder.project_dir().join("2026-09-18.jsonl");
        assert_eq!(
            fs::metadata(&file).expect("file").permissions().mode() & 0o777,
            0o600
        );
        for dir in [
            recorder.project_dir(),
            root.join("parallel_dependency"),
            root.clone(),
        ] {
            assert_eq!(
                fs::metadata(&dir).expect("dir").permissions().mode() & 0o777,
                0o700,
                "{} must be private",
                dir.display()
            );
        }
    }

    #[test]
    fn a_corrupt_salt_fails_only_the_observation() {
        let base = tempfile::tempdir().expect("temp base");
        let root = base.path().join("evaluations");
        super::super::fsutil::create_private_dir_all(&root).expect("root");
        fs::write(salt_path(&root), b"too short").expect("corrupt salt");

        let error = recorder(&root)
            .record_observation(&observation())
            .expect_err("a corrupt salt must fail the write");
        assert!(matches!(error, EvaluationError::Salt { .. }), "{error:?}");

        // A failure record needs no salt, so it still lands: the degradation is
        // scoped to what actually depended on the broken input.
        recorder(&root)
            .record_failure("io", 5, 1)
            .expect("failure record needs no pair identity");
        assert_eq!(read_records(&root).len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn an_unwritable_root_degrades_to_a_typed_error() {
        use std::os::unix::fs::PermissionsExt;

        let base = tempfile::tempdir().expect("temp base");
        let locked = base.path().join("locked");
        fs::create_dir(&locked).expect("create");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).expect("lock");

        let error = recorder(&locked.join("evaluations"))
            .record_failure("io", 1, 1)
            .expect_err("an unwritable root must fail the write");
        assert!(
            matches!(
                error,
                EvaluationError::Directory { .. } | EvaluationError::Salt { .. }
            ),
            "{error:?}"
        );

        // Restore so the temporary directory can be cleaned up.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).expect("unlock");
    }

    #[test]
    fn disabled_configuration_builds_no_recorder_at_all() {
        use crate::config::{
            JudgeCommandsConfig, JudgeEvaluationConfig, OrchestratorConfig,
            ParallelDependencyJudgeConfig,
        };

        let judge = |evaluation: Option<JudgeEvaluationConfig>| ParallelDependencyJudgeConfig {
            command: vec!["jev".to_string(), "run".to_string(), "-".to_string()],
            model: "jev-1.13.0".to_string(),
            timeout_ms: None,
            max_input_bytes: None,
            max_output_bytes: None,
            yes_threshold: None,
            mode: None,
            evaluation,
        };
        let config = |judge: Option<ParallelDependencyJudgeConfig>| OrchestratorConfig {
            judge_commands: judge.map(|judge| JudgeCommandsConfig {
                parallel_dependency: Some(judge),
            }),
            ..Default::default()
        };

        let repo_root = tempfile::tempdir().expect("repo root");
        let cases = [
            ("no judge entry at all", config(None)),
            ("judge without evaluation", config(Some(judge(None)))),
            (
                "evaluation explicitly disabled",
                config(Some(judge(Some(JudgeEvaluationConfig {
                    enabled: false,
                    retention_days: Some(30),
                    max_total_bytes: Some(1024),
                })))),
            ),
            (
                "enabled but incomplete bounds",
                config(Some(judge(Some(JudgeEvaluationConfig {
                    enabled: true,
                    retention_days: None,
                    max_total_bytes: Some(1024),
                })))),
            ),
        ];

        for (label, config) in cases {
            assert!(
                EvaluationRecorder::from_config(&config, repo_root.path()).is_none(),
                "{label} must not produce a recorder"
            );
        }
    }

    #[test]
    fn enabled_configuration_builds_a_recorder_with_its_bounds() {
        use crate::config::{
            JudgeCommandsConfig, JudgeEvaluationConfig, OrchestratorConfig,
            ParallelDependencyJudgeConfig,
        };

        let state_root = tempfile::tempdir().expect("state root");
        let repo_root = tempfile::tempdir().expect("repo root");
        let config = OrchestratorConfig {
            state_base_dir: Some(state_root.path().display().to_string()),
            judge_commands: Some(JudgeCommandsConfig {
                parallel_dependency: Some(ParallelDependencyJudgeConfig {
                    command: vec!["jev".to_string()],
                    model: "jev-1.13.0".to_string(),
                    timeout_ms: None,
                    max_input_bytes: None,
                    max_output_bytes: None,
                    yes_threshold: None,
                    mode: Some("shadow".to_string()),
                    evaluation: Some(JudgeEvaluationConfig {
                        enabled: true,
                        retention_days: Some(45),
                        max_total_bytes: Some(4096),
                    }),
                }),
            }),
            ..Default::default()
        };

        let recorder = EvaluationRecorder::from_config(&config, repo_root.path())
            .expect("an enabled policy must produce a recorder");

        assert_eq!(
            recorder.policy(),
            EvaluationPolicy {
                retention_days: 45,
                max_total_bytes: 4096
            }
        );
        // The evaluation root is a sibling of `logs`, never a descendant: that
        // separation is what keeps log retention and the log viewer unable to
        // reach evaluation records.
        assert!(recorder.root().ends_with("cflx/evaluations"));
        assert!(!recorder.root().starts_with(
            crate::config::defaults::log_root_path(config.get_state_base_dir()).expect("log root")
        ));
        // Nothing was created merely by building the recorder.
        assert!(!recorder.root().exists());
    }
}
