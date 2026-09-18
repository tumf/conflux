//! Read-only aggregation of evaluation records.
//!
//! This is the whole point of recording: precision, recall, specificity,
//! latency, cost, and outcome rates that a promotion decision can be argued
//! from. It is strictly read-only — it creates no root, no salt, and no daily
//! file, runs no cleanup, and initializes no ordinary logging — because an
//! inspection that mutates the thing it inspects is not evidence.
//!
//! Truthfulness beats completeness: a malformed line or an unsupported schema
//! version is *counted as skipped* and excluded from every metric. Silently
//! dropping it would overstate the sample; guessing at it would invent data.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::error::{EvaluationError, EvaluationResult};
use super::record::{EvaluationRecord, EVALUATION_SCHEMA_VERSION};

/// Version of the JSON summary contract itself, independent of the record
/// schema version: a reader can gain fields without the records changing.
pub const SUMMARY_SCHEMA_VERSION: u32 = 1;

/// Pair-level confusion counts, with the judge as the predictor and the
/// conventional analyzer as ground truth.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfusionCounts {
    pub true_positive: u64,
    pub false_positive: u64,
    pub false_negative: u64,
    pub true_negative: u64,
}

impl ConfusionCounts {
    /// `TP / (TP + FP)`, or `None` when the judge predicted no positives.
    ///
    /// Nullable rather than zero on purpose: "the judge was never right" and
    /// "the judge never guessed" are different findings, and collapsing them
    /// into `0.0` would make an empty sample look like a failing one.
    pub fn precision(&self) -> Option<f64> {
        ratio(self.true_positive, self.true_positive + self.false_positive)
    }

    /// `TP / (TP + FN)`, or `None` when the analyzer found no dependencies.
    pub fn recall(&self) -> Option<f64> {
        ratio(self.true_positive, self.true_positive + self.false_negative)
    }

    /// `TN / (TN + FP)`, or `None` when the analyzer found no independent pairs.
    pub fn specificity(&self) -> Option<f64> {
        ratio(self.true_negative, self.true_negative + self.false_positive)
    }

    fn total(&self) -> u64 {
        self.true_positive + self.false_positive + self.false_negative + self.true_negative
    }
}

fn ratio(numerator: u64, denominator: u64) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

/// One aggregated evaluation summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationSummary {
    pub schema_version: u32,
    pub purpose: String,

    /// Inclusive lower bound on record file dates, when one was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,

    /// Exact project filter, when one was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,

    pub files_examined: usize,
    pub lines_examined: usize,

    /// Valid schema-v1 observation records.
    pub observations: u64,
    /// Valid schema-v1 failure records.
    pub failures: u64,
    /// Lines that were not valid schema-v1 records and contributed to nothing.
    pub skipped_malformed: u64,
    /// Lines that parsed but carry a schema version this build cannot read.
    pub skipped_unsupported_schema: u64,

    /// Record counts per outcome token, ordered for byte-stable output.
    pub outcome_counts: BTreeMap<String, u64>,
    /// Each outcome's share of all valid records.
    pub outcome_rates: BTreeMap<String, f64>,

    pub confusion: ConfusionCounts,
    pub pairs: u64,

    pub precision: Option<f64>,
    pub recall: Option<f64>,
    pub specificity: Option<f64>,

    /// Nearest-rank p50 over every valid record's duration.
    pub duration_p50_ms: Option<u64>,
    /// Nearest-rank p95 over every valid record's duration.
    pub duration_p95_ms: Option<u64>,

    pub input_tokens: u64,
    pub output_tokens: u64,

    /// Observation counts by validated model.
    pub by_model: BTreeMap<String, u64>,
    /// Valid-record counts by the Conflux version that produced them.
    pub by_conflux_version: BTreeMap<String, u64>,
}

impl EvaluationSummary {
    fn empty(purpose: &str, since: Option<NaiveDate>, project: Option<&str>) -> Self {
        Self {
            schema_version: SUMMARY_SCHEMA_VERSION,
            purpose: purpose.to_string(),
            since: since.map(|date| date.format("%Y-%m-%d").to_string()),
            project: project.map(str::to_string),
            files_examined: 0,
            lines_examined: 0,
            observations: 0,
            failures: 0,
            skipped_malformed: 0,
            skipped_unsupported_schema: 0,
            outcome_counts: BTreeMap::new(),
            outcome_rates: BTreeMap::new(),
            confusion: ConfusionCounts::default(),
            pairs: 0,
            precision: None,
            recall: None,
            specificity: None,
            duration_p50_ms: None,
            duration_p95_ms: None,
            input_tokens: 0,
            output_tokens: 0,
            by_model: BTreeMap::new(),
            by_conflux_version: BTreeMap::new(),
        }
    }

    /// Total valid records of either kind.
    pub fn valid_records(&self) -> u64 {
        self.observations + self.failures
    }
}

/// What a summary run was asked to read.
#[derive(Debug, Clone)]
pub struct SummaryQuery {
    /// Judgment purpose whose store is read. Selects the directory *and* the
    /// reported `purpose`, so the two can never disagree.
    pub purpose: String,
    /// Inclusive lower bound on the record file's date, or `None` for all.
    pub since: Option<NaiveDate>,
    /// Exact project slug, or `None` for every project directory.
    pub project: Option<String>,
}

impl SummaryQuery {
    /// Read every record of the default purpose.
    #[cfg(test)]
    fn all() -> Self {
        Self {
            purpose: crate::config::defaults::EVALUATION_PURPOSE_PARALLEL_DEPENDENCY.to_string(),
            since: None,
            project: None,
        }
    }
}

/// Read and aggregate every selected record beneath `root`.
///
/// An absent root or purpose directory is an empty summary, not an error: there
/// is nothing to report and nothing to create.
pub fn summarize(root: &Path, query: &SummaryQuery) -> EvaluationResult<EvaluationSummary> {
    let mut summary =
        EvaluationSummary::empty(&query.purpose, query.since, query.project.as_deref());

    let files = select_files(root, query)?;
    summary.files_examined = files.len();

    let mut durations: Vec<u64> = Vec::new();

    for file in files {
        let content = match std::fs::read_to_string(&file) {
            Ok(content) => content,
            // An unreadable individual file is reported the same way a
            // malformed line is: it is missing evidence, not a reason to
            // abandon the files that did read.
            Err(_) => {
                summary.skipped_malformed += 1;
                continue;
            }
        };

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            summary.lines_examined += 1;

            let Ok(record) = serde_json::from_str::<EvaluationRecord>(line) else {
                summary.skipped_malformed += 1;
                continue;
            };
            if record.schema_version != EVALUATION_SCHEMA_VERSION {
                summary.skipped_unsupported_schema += 1;
                continue;
            }

            accumulate(&mut summary, &mut durations, record);
        }
    }

    finish(&mut summary, durations);
    Ok(summary)
}

/// Fold one valid record into the running totals.
fn accumulate(summary: &mut EvaluationSummary, durations: &mut Vec<u64>, record: EvaluationRecord) {
    *summary
        .outcome_counts
        .entry(record.outcome.clone())
        .or_insert(0) += 1;
    *summary
        .by_conflux_version
        .entry(record.conflux_version.clone())
        .or_insert(0) += 1;
    durations.push(record.duration_ms);

    if !record.is_observed() {
        summary.failures += 1;
        return;
    }

    summary.observations += 1;
    summary.input_tokens = summary
        .input_tokens
        .saturating_add(record.input_tokens.unwrap_or(0));
    summary.output_tokens = summary
        .output_tokens
        .saturating_add(record.output_tokens.unwrap_or(0));
    if let Some(model) = record.model {
        *summary.by_model.entry(model).or_insert(0) += 1;
    }

    for pair in record.pairs.into_iter().flatten() {
        match (pair.judge_dependency, pair.analyzer_dependency) {
            (true, true) => summary.confusion.true_positive += 1,
            (true, false) => summary.confusion.false_positive += 1,
            (false, true) => summary.confusion.false_negative += 1,
            (false, false) => summary.confusion.true_negative += 1,
        }
    }
}

/// Derive the rates and percentiles that depend on the complete sample.
fn finish(summary: &mut EvaluationSummary, mut durations: Vec<u64>) {
    let valid = summary.valid_records();
    if valid > 0 {
        for (outcome, count) in &summary.outcome_counts {
            summary
                .outcome_rates
                .insert(outcome.clone(), *count as f64 / valid as f64);
        }
    }

    summary.pairs = summary.confusion.total();
    summary.precision = summary.confusion.precision();
    summary.recall = summary.confusion.recall();
    summary.specificity = summary.confusion.specificity();

    durations.sort_unstable();
    summary.duration_p50_ms = nearest_rank(&durations, 50);
    summary.duration_p95_ms = nearest_rank(&durations, 95);
}

/// Nearest-rank percentile over a sorted sample.
///
/// Nearest-rank rather than an interpolating definition because the result must
/// be an observed value and must be reproducible from the stored records alone
/// — two readers must never disagree about p95 because they round differently.
fn nearest_rank(sorted: &[u64], percentile: u64) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    // ceil(p/100 * n), clamped into 1..=n.
    let n = sorted.len() as u64;
    let rank = (percentile * n).div_ceil(100).max(1).min(n);
    Some(sorted[(rank - 1) as usize])
}

/// Every recognized record file the query selects, in deterministic order.
fn select_files(root: &Path, query: &SummaryQuery) -> EvaluationResult<Vec<PathBuf>> {
    let purpose_dir = root.join(&query.purpose);
    if !purpose_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut selected: Vec<(String, PathBuf)> = Vec::new();

    let projects =
        std::fs::read_dir(&purpose_dir).map_err(|source| EvaluationError::Read { source })?;
    for project in projects.flatten() {
        let name = project.file_name().to_string_lossy().into_owned();
        if let Some(wanted) = query.project.as_deref() {
            if name != wanted {
                continue;
            }
        }
        let project_path = project.path();
        if !project_path.is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&project_path) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if !path.is_file() {
                continue;
            }
            let Some(date) = record_date(&path) else {
                continue;
            };
            if query.since.is_some_and(|since| date < since) {
                continue;
            }
            selected.push((format!("{name}/{date}"), path));
        }
    }

    // Sorted by project then date, so repeated reads of the same store produce
    // byte-identical output.
    selected.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(selected.into_iter().map(|(_, path)| path).collect())
}

fn record_date(path: &Path) -> Option<NaiveDate> {
    if path.extension()?.to_str()? != "jsonl" {
        return None;
    }
    NaiveDate::parse_from_str(path.file_stem()?.to_str()?, "%Y-%m-%d").ok()
}

/// Lower date bound for a `--since-days N` window ending today.
pub fn since_from_days(days: u32, today: DateTime<Utc>) -> NaiveDate {
    today.date_naive() - chrono::Duration::days(i64::from(days.saturating_sub(1)))
}

/// Render a rate that may have had no denominator.
///
/// `n/a` rather than `0.000`, for the same reason the field is nullable: an
/// operator reading a promotion metric must be able to tell "the judge was
/// wrong" from "the question was never asked".
fn render_rate(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_string(), |rate| format!("{rate:.3}"))
}

fn render_duration(value: Option<u64>) -> String {
    value.map_or_else(|| "n/a".to_string(), |ms| format!("{ms} ms"))
}

/// Write the concise human-readable form of a summary.
pub fn render_human(
    summary: &EvaluationSummary,
    out: &mut impl std::io::Write,
) -> std::io::Result<()> {
    writeln!(out, "purpose: {}", summary.purpose)?;
    if let Some(since) = &summary.since {
        writeln!(out, "since:   {since}")?;
    }
    if let Some(project) = &summary.project {
        writeln!(out, "project: {project}")?;
    }
    writeln!(
        out,
        "files:   {} ({} lines)",
        summary.files_examined, summary.lines_examined
    )?;
    writeln!(
        out,
        "records: {} valid ({} observed, {} failed), {} skipped malformed, {} skipped unsupported schema",
        summary.valid_records(),
        summary.observations,
        summary.failures,
        summary.skipped_malformed,
        summary.skipped_unsupported_schema
    )?;

    if summary.valid_records() == 0 {
        writeln!(out, "\nNo evaluation records matched.")?;
        return Ok(());
    }

    writeln!(out, "\noutcomes:")?;
    for (outcome, count) in &summary.outcome_counts {
        let rate = summary.outcome_rates.get(outcome).copied().unwrap_or(0.0);
        writeln!(out, "  {outcome:<18} {count:>6}  ({rate:.3})")?;
    }

    writeln!(
        out,
        "\npairs:   {} (TP {} / FP {} / FN {} / TN {})",
        summary.pairs,
        summary.confusion.true_positive,
        summary.confusion.false_positive,
        summary.confusion.false_negative,
        summary.confusion.true_negative
    )?;
    writeln!(
        out,
        "  precision {}   recall {}   specificity {}",
        render_rate(summary.precision),
        render_rate(summary.recall),
        render_rate(summary.specificity)
    )?;

    writeln!(
        out,
        "\nduration: p50 {}   p95 {}",
        render_duration(summary.duration_p50_ms),
        render_duration(summary.duration_p95_ms)
    )?;
    writeln!(
        out,
        "tokens:   {} in / {} out",
        summary.input_tokens, summary.output_tokens
    )?;

    if !summary.by_model.is_empty() {
        writeln!(out, "\nby model:")?;
        for (model, count) in &summary.by_model {
            writeln!(out, "  {model:<28} {count:>6}")?;
        }
    }
    if !summary.by_conflux_version.is_empty() {
        writeln!(out, "\nby conflux version:")?;
        for (version, count) in &summary.by_conflux_version {
            writeln!(out, "  {version:<28} {count:>6}")?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge_evaluation::record::{EvaluationPairRecord, OUTCOME_OBSERVED};
    use std::fs;

    const PURPOSE: &str = "parallel_dependency";

    fn observed(
        pairs: &[(bool, bool)],
        duration_ms: u64,
        model: &str,
        version: &str,
    ) -> EvaluationRecord {
        EvaluationRecord {
            schema_version: EVALUATION_SCHEMA_VERSION,
            recorded_at: "2026-09-18T00:00:00.000Z".to_string(),
            conflux_version: version.to_string(),
            purpose: crate::config::defaults::EVALUATION_PURPOSE_PARALLEL_DEPENDENCY.to_string(),
            mode: "shadow".to_string(),
            outcome: OUTCOME_OBSERVED.to_string(),
            duration_ms,
            expected_answers: pairs.len(),
            model: Some(model.to_string()),
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

    fn failure(outcome: &str, duration_ms: u64, version: &str) -> EvaluationRecord {
        EvaluationRecord {
            model: None,
            returned_answers: None,
            input_tokens: None,
            output_tokens: None,
            yes_threshold: None,
            pairs: None,
            outcome: outcome.to_string(),
            duration_ms,
            conflux_version: version.to_string(),
            ..observed(&[], 0, "unused", version)
        }
    }

    fn write_lines(root: &Path, project: &str, date: &str, lines: &[String]) {
        let dir = root.join(PURPOSE).join(project);
        fs::create_dir_all(&dir).expect("project dir");
        fs::write(dir.join(format!("{date}.jsonl")), lines.join("\n") + "\n").expect("file");
    }

    fn write_records(root: &Path, project: &str, date: &str, records: &[EvaluationRecord]) {
        let lines: Vec<String> = records
            .iter()
            .map(|record| serde_json::to_string(record).expect("serialize"))
            .collect();
        write_lines(root, project, date, &lines);
    }

    fn all() -> SummaryQuery {
        SummaryQuery::all()
    }

    #[test]
    fn confusion_metrics_and_totals_equal_the_fixture_values() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        write_records(
            &root,
            "proj-a",
            "2026-09-18",
            &[
                // TP, FP, FN, TN
                observed(&[(true, true), (true, false)], 100, "jev-1.13.0", "0.6.1"),
                observed(&[(false, true), (false, false)], 300, "jev-1.13.0", "0.6.1"),
            ],
        );

        let summary = summarize(&root, &all()).expect("summary");

        assert_eq!(summary.observations, 2);
        assert_eq!(summary.failures, 0);
        assert_eq!(summary.lines_examined, 2);
        assert_eq!(summary.files_examined, 1);
        assert_eq!(
            summary.confusion,
            ConfusionCounts {
                true_positive: 1,
                false_positive: 1,
                false_negative: 1,
                true_negative: 1,
            }
        );
        assert_eq!(summary.pairs, 4);
        assert_eq!(summary.precision, Some(0.5));
        assert_eq!(summary.recall, Some(0.5));
        assert_eq!(summary.specificity, Some(0.5));
        assert_eq!(summary.input_tokens, 200);
        assert_eq!(summary.output_tokens, 20);
        assert_eq!(summary.by_model.get("jev-1.13.0"), Some(&2));
        assert_eq!(summary.by_conflux_version.get("0.6.1"), Some(&2));
    }

    #[test]
    fn rates_are_null_rather_than_zero_when_the_denominator_is_empty() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        // The judge said no to everything the analyzer also said no to: there
        // is no positive prediction and no true dependency at all.
        write_records(
            &root,
            "proj-a",
            "2026-09-18",
            &[observed(&[(false, false), (false, false)], 10, "m", "v")],
        );

        let summary = summarize(&root, &all()).expect("summary");
        assert_eq!(summary.precision, None, "no positive prediction was made");
        assert_eq!(summary.recall, None, "there was nothing to recall");
        assert_eq!(summary.specificity, Some(1.0), "but specificity is defined");
    }

    #[test]
    fn nearest_rank_percentiles_are_exact_and_deterministic() {
        // Rank = ceil(p/100 * n): for n = 10, p50 is the 5th value and p95 the
        // 10th. These are observed values, never interpolations.
        let sorted: Vec<u64> = (1..=10).map(|value| value * 10).collect();
        assert_eq!(nearest_rank(&sorted, 50), Some(50));
        assert_eq!(nearest_rank(&sorted, 95), Some(100));

        assert_eq!(nearest_rank(&[], 50), None, "an empty sample has no median");
        assert_eq!(nearest_rank(&[7], 50), Some(7));
        assert_eq!(nearest_rank(&[7], 95), Some(7));
        assert_eq!(nearest_rank(&[1, 2], 50), Some(1));
        assert_eq!(nearest_rank(&[1, 2], 95), Some(2));
    }

    #[test]
    fn percentiles_cover_failures_as_well_as_observations() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        let records: Vec<EvaluationRecord> = (1..=10)
            .map(|index| {
                if index % 2 == 0 {
                    failure("timeout", index * 10, "v")
                } else {
                    observed(&[(true, true)], index * 10, "m", "v")
                }
            })
            .collect();
        write_records(&root, "proj-a", "2026-09-18", &records);

        let summary = summarize(&root, &all()).expect("summary");
        assert_eq!(summary.valid_records(), 10);
        assert_eq!(summary.duration_p50_ms, Some(50));
        assert_eq!(summary.duration_p95_ms, Some(100));
    }

    #[test]
    fn outcome_counts_and_rates_partition_the_valid_sample() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        write_records(
            &root,
            "proj-a",
            "2026-09-18",
            &[
                observed(&[(true, true)], 10, "m", "v"),
                observed(&[(true, true)], 10, "m", "v"),
                failure("timeout", 20, "v"),
                failure("spawn", 30, "v"),
            ],
        );

        let summary = summarize(&root, &all()).expect("summary");
        assert_eq!(summary.outcome_counts.get("observed"), Some(&2));
        assert_eq!(summary.outcome_counts.get("timeout"), Some(&1));
        assert_eq!(summary.outcome_counts.get("spawn"), Some(&1));
        assert_eq!(summary.outcome_rates.get("observed"), Some(&0.5));
        assert_eq!(summary.outcome_rates.get("timeout"), Some(&0.25));

        let total: f64 = summary.outcome_rates.values().sum();
        assert!(
            (total - 1.0).abs() < 1e-12,
            "rates must sum to one: {total}"
        );
    }

    #[test]
    fn malformed_and_future_schema_lines_are_counted_but_excluded() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");

        let valid = serde_json::to_string(&observed(&[(true, true)], 100, "good", "0.6.1"))
            .expect("serialize");
        let future = serde_json::to_string(&EvaluationRecord {
            schema_version: EVALUATION_SCHEMA_VERSION + 1,
            duration_ms: 999_999,
            ..observed(&[(false, false)], 999_999, "future-model", "9.9.9")
        })
        .expect("serialize");

        write_lines(
            &root,
            "proj-a",
            "2026-09-18",
            &[
                valid,
                "{not json at all".to_string(),
                "[]".to_string(),
                r#"{"schema_version":1}"#.to_string(),
                future,
                "   ".to_string(),
            ],
        );

        let summary = summarize(&root, &all()).expect("summary");

        assert_eq!(summary.lines_examined, 5, "blank lines are not records");
        assert_eq!(summary.skipped_malformed, 3);
        assert_eq!(summary.skipped_unsupported_schema, 1);
        assert_eq!(summary.observations, 1);
        assert_eq!(summary.failures, 0);
        // Nothing from the excluded lines reaches any metric.
        assert_eq!(summary.pairs, 1);
        assert_eq!(summary.duration_p50_ms, Some(100));
        assert_eq!(summary.duration_p95_ms, Some(100));
        assert!(!summary.by_model.contains_key("future-model"));
        assert!(!summary.by_conflux_version.contains_key("9.9.9"));
        assert_eq!(summary.outcome_counts.len(), 1);
    }

    #[test]
    fn repeated_reads_produce_byte_identical_json() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        write_records(
            &root,
            "proj-b",
            "2026-09-17",
            &[observed(&[(true, false)], 40, "m2", "v2")],
        );
        write_records(
            &root,
            "proj-a",
            "2026-09-18",
            &[observed(&[(true, true)], 10, "m1", "v1")],
        );

        let first =
            serde_json::to_string(&summarize(&root, &all()).expect("first")).expect("serialize");
        for _ in 0..3 {
            let again = serde_json::to_string(&summarize(&root, &all()).expect("again"))
                .expect("serialize");
            assert_eq!(
                first, again,
                "no field may depend on read order or the clock"
            );
        }
        // No timestamp field exists at all, which is what makes the above true.
        assert!(!first.contains("generated_at"));
    }

    #[test]
    fn summary_is_read_only_and_creates_nothing() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");

        let summary = summarize(&root, &all()).expect("a missing root is an empty summary");
        assert!(
            !root.exists(),
            "summary must not create the evaluation root"
        );
        assert_eq!(summary.files_examined, 0);
        assert_eq!(summary.lines_examined, 0);
        assert_eq!(summary.valid_records(), 0);
        assert_eq!(summary.precision, None);
        assert_eq!(summary.duration_p50_ms, None);
        assert!(summary.outcome_counts.is_empty());

        // And with a real store, the bytes on disk are unchanged.
        write_records(
            &root,
            "proj-a",
            "2026-09-18",
            &[observed(&[(true, true)], 10, "m", "v")],
        );
        let file = root.join(PURPOSE).join("proj-a").join("2026-09-18.jsonl");
        let before = fs::read(&file).expect("read");
        summarize(&root, &all()).expect("summary");
        assert_eq!(fs::read(&file).expect("read"), before);
        assert!(
            !root
                .join(crate::config::defaults::EVALUATION_SALT_FILE_NAME)
                .exists(),
            "summary must never create the installation salt"
        );
    }

    #[test]
    fn project_and_since_filters_select_exactly_their_records() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        write_records(
            &root,
            "proj-a",
            "2026-09-18",
            &[observed(&[(true, true)], 10, "m", "v")],
        );
        write_records(
            &root,
            "proj-a",
            "2026-09-01",
            &[observed(&[(true, true)], 10, "m", "v")],
        );
        write_records(
            &root,
            "proj-b",
            "2026-09-18",
            &[observed(&[(true, true)], 10, "m", "v")],
        );

        let by_project = summarize(
            &root,
            &SummaryQuery {
                project: Some("proj-a".to_string()),
                ..SummaryQuery::all()
            },
        )
        .expect("summary");
        assert_eq!(by_project.files_examined, 2);
        assert_eq!(by_project.project.as_deref(), Some("proj-a"));

        let by_date = summarize(
            &root,
            &SummaryQuery {
                since: NaiveDate::from_ymd_opt(2026, 9, 10),
                ..SummaryQuery::all()
            },
        )
        .expect("summary");
        assert_eq!(
            by_date.files_examined, 2,
            "the September 1 file is excluded"
        );
        assert_eq!(by_date.since.as_deref(), Some("2026-09-10"));

        let both = summarize(
            &root,
            &SummaryQuery {
                since: NaiveDate::from_ymd_opt(2026, 9, 10),
                project: Some("proj-b".to_string()),
                ..SummaryQuery::all()
            },
        )
        .expect("summary");
        assert_eq!(both.files_examined, 1);

        let missing_project = summarize(
            &root,
            &SummaryQuery {
                project: Some("proj-absent".to_string()),
                ..SummaryQuery::all()
            },
        )
        .expect("an unmatched project is empty, not an error");
        assert_eq!(missing_project.files_examined, 0);
    }

    #[test]
    fn no_pair_identifier_reaches_the_summary_output() {
        let base = tempfile::tempdir().expect("base");
        let root = base.path().join("evaluations");
        write_records(
            &root,
            "proj-a",
            "2026-09-18",
            &[observed(&[(true, true), (false, false)], 10, "m", "v")],
        );

        let rendered =
            serde_json::to_string(&summarize(&root, &all()).expect("summary")).expect("serialize");
        assert!(
            !rendered.contains("sha256:"),
            "pair identifiers must stay out of summary output: {rendered}"
        );
        assert!(!rendered.contains("pair_id"));
    }

    #[test]
    fn since_from_days_window_includes_today() {
        let today = DateTime::parse_from_rfc3339("2026-09-18T00:00:00Z")
            .expect("instant")
            .with_timezone(&Utc);
        assert_eq!(
            since_from_days(1, today),
            NaiveDate::from_ymd_opt(2026, 9, 18).expect("date"),
            "a one-day window is today alone"
        );
        assert_eq!(
            since_from_days(30, today),
            NaiveDate::from_ymd_opt(2026, 8, 20).expect("date")
        );
    }
}
