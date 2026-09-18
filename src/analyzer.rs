//! Parallelization analyzer for identifying independent changes.
//!
//! Uses LLM-based analysis to determine which changes can be executed
//! in parallel and what dependencies exist between them.

use crate::ai_command_runner::OutputLine as AiOutputLine;
use crate::config::ParallelDependencyJudgeConfig;
use crate::dependency_targets::{
    classify_dependency_target, collect_active_change_ids, collect_archived_change_ids,
    collect_rejected_change_ids, union_metadata_dependencies, DependencyTargetClass,
};
use crate::error::{OrchestratorError, Result};
use crate::judge_command::{
    parse_system_one_response, spawn_judge, JudgeChangeNode, JudgeCollection, JudgeCommandSpec,
    JudgeFailureCategory, JudgeHandle, JudgeRequestState, JudgeUsage, NoulCriteria, NoulQuestion,
    SystemOneRequest, JUDGE_STATE_SCHEMA_VERSION, NOUL_QUESTION_TYPE, PARALLEL_DEPENDENCY_PURPOSE,
};
use crate::openspec::{Change, ProposalFrontmatterMetadata};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;
use tracing::{debug, info, warn};

/// A group of changes that can be executed in parallel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelGroup {
    /// Group identifier
    pub id: u32,
    /// Change IDs in this group
    pub changes: Vec<String>,
    /// Group IDs this group depends on (must complete before this group starts)
    #[serde(default)]
    pub depends_on: Vec<u32>,
}

/// Result of parallelization analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Execution order (recommended execution sequence considering dependencies)
    pub order: Vec<String>,
    /// Dependencies between changes (change_id -> list of dependencies)
    #[serde(default)]
    pub dependencies: HashMap<String, Vec<String>>,
    /// Legacy groups field (deprecated, for backward compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub groups: Option<Vec<ParallelGroup>>,
}

/// How an [`AnalysisResult`] was produced, as runtime-only execution metadata.
///
/// The scheduler needs to tell a *completed* analysis apart from a *degraded* one: a
/// healthy or intentionally metadata-only result describes the input faithfully and may
/// suppress an unchanged ordinary timer input indefinitely, while a metadata fallback
/// produced by a recoverable LLM failure must only suppress it for a bounded interval so
/// a transient analyzer outage still recovers.
///
/// This provenance is process-local and is never serialized, persisted, or used as
/// authoritative workflow state; it only decides whether duplicate analyzer work is
/// avoided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisProvenance {
    /// The configured LLM analysis command produced the result.
    HealthyLlm,
    /// LLM analysis is intentionally disabled, so metadata-dependency analysis is the
    /// configured result rather than a fallback.
    IntentionalMetadataOnly,
    /// LLM analysis failed recoverably and the metadata-dependency fallback was used.
    RecoverableFailureFallback,
}

impl AnalysisProvenance {
    /// Whether this result is a recoverable-failure fallback rather than the analysis the
    /// current configuration asks for.
    pub fn is_degraded(self) -> bool {
        matches!(self, AnalysisProvenance::RecoverableFailureFallback)
    }
}

/// An [`AnalysisResult`] together with the runtime-only provenance of how it was produced.
///
/// The dependency semantics carried by `result` are unchanged; `provenance` is additional
/// execution metadata for the scheduler.
#[derive(Debug, Clone)]
pub struct AnalysisOutcome {
    pub result: AnalysisResult,
    pub provenance: AnalysisProvenance,
}

impl AnalysisOutcome {
    pub fn new(result: AnalysisResult, provenance: AnalysisProvenance) -> Self {
        Self { result, provenance }
    }

    pub fn healthy(result: AnalysisResult) -> Self {
        Self::new(result, AnalysisProvenance::HealthyLlm)
    }

    pub fn intentional_metadata_only(result: AnalysisResult) -> Self {
        Self::new(result, AnalysisProvenance::IntentionalMetadataOnly)
    }

    pub fn recoverable_failure_fallback(result: AnalysisResult) -> Self {
        Self::new(result, AnalysisProvenance::RecoverableFailureFallback)
    }
}

/// Callers that do not model analyzer health treat their result as healthy output.
impl From<AnalysisResult> for AnalysisOutcome {
    fn from(result: AnalysisResult) -> Self {
        Self::healthy(result)
    }
}

// ── Shadow dependency judge ────────────────────────────────────────────────

/// Noul `criteria.true` for every dependency question.
const JUDGE_TRUE_CRITERIA: &str = "The source change, contract, schema, migration, configuration, \
     test surface, specification, or generated artifact produced by the candidate dependency is \
     required.";

/// Noul `criteria.false` for every dependency question.
///
/// Priority, references, and mere topical relation are named explicitly so they
/// are excluded as dependency evidence, exactly as the conventional analyzer
/// prompt excludes them.
const JUDGE_FALSE_CRITERIA: &str = "The changes are independent, merely related, ordered only by \
     preference or priority, or connected only by a reference.";

/// Question ID → `(dependent, dependency)` change pair.
///
/// This map stays in memory and is the only parser authority: change IDs never
/// become JSON object keys, so nothing a proposal author writes can alter the
/// protocol's structure.
type JudgePairMap = BTreeMap<String, (String, String)>;

/// A built shadow request together with its trusted local pair map.
type ShadowJudgeRequest = (SystemOneRequest, JudgePairMap);

/// One in-flight shadow judge invocation and the trusted local state needed to
/// interpret its answer.
struct ShadowDependencyJudgeRun {
    handle: JudgeHandle,
    pairs: JudgePairMap,
    model: String,
    yes_threshold: f64,
}

/// A completed, strictly validated, *non-authoritative* judge observation.
///
/// Process-local and ephemeral: it is never serialized, persisted, or consumed
/// by [`AnalysisProvenance`], dispatch, retry debounce, blocker classification,
/// acceptance, or archive decisions.
struct DependencyJudgeObservation {
    /// Model identity, already proven equal to the requested model.
    model: String,
    /// Noul probability per `(dependent, dependency)` pair.
    values: HashMap<(String, String), f64>,
    usage: JudgeUsage,
    duration: Duration,
    /// Threshold used to project probabilities onto comparison edges.
    yes_threshold: f64,
}

/// Stable lowercase token for a proposal priority.
fn priority_token(priority: crate::openspec::ProposalPriority) -> &'static str {
    match priority {
        crate::openspec::ProposalPriority::High => "high",
        crate::openspec::ProposalPriority::Medium => "medium",
        crate::openspec::ProposalPriority::Low => "low",
    }
}

/// Whether a change ID may be used to build a proposal path for the judge.
///
/// Conservative on purpose: only an ID that cannot traverse, escape, or name a
/// hidden entry is accepted, so an untrusted ID can never widen the single
/// `repo_root/openspec/changes/<id>/proposal.md` layout the judge reads from.
fn is_judge_safe_change_id(change_id: &str) -> bool {
    !change_id.is_empty()
        && change_id.len() <= 200
        && !change_id.starts_with('.')
        && change_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !change_id.contains("..")
}

/// Analyzer for determining parallel execution groups
pub struct ParallelizationAnalyzer {
    ai_runner: crate::ai_command_runner::AiCommandRunner,
    config: crate::config::OrchestratorConfig,
    /// Repository root this analyzer reads proposal files from.
    ///
    /// Held explicitly rather than inferred from the process working directory
    /// or a `git rev-parse` during analysis: the caller already knows which
    /// repository it is analyzing, and a shadow judge must never be able to
    /// read a proposal from somewhere else.
    repo_root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AnalyzePromptMetadata {
    priority: Option<String>,
    references: Vec<String>,
    warnings: Vec<String>,
}

impl AnalyzePromptMetadata {
    fn from_frontmatter(metadata: &ProposalFrontmatterMetadata) -> Self {
        Self {
            priority: metadata.priority.clone(),
            references: metadata.references.clone(),
            warnings: metadata
                .warnings
                .iter()
                .map(|warning| warning.message.clone())
                .collect(),
        }
    }
}

impl ParallelizationAnalyzer {
    /// Create a new analyzer with the given AI command runner, configuration,
    /// and the repository root whose proposals it may read.
    pub fn new(
        ai_runner: crate::ai_command_runner::AiCommandRunner,
        config: crate::config::OrchestratorConfig,
        repo_root: PathBuf,
    ) -> Self {
        Self {
            ai_runner,
            config,
            repo_root,
        }
    }

    /// Analyze changes and return the raw analysis result with order and dependencies.
    ///
    /// This is the preferred method for order-based execution.
    /// Returns the analysis result containing:
    /// - `order`: Recommended execution sequence considering dependencies
    /// - `dependencies`: Change-level dependency constraints
    #[allow(dead_code)]
    pub async fn analyze(&self, changes: &[Change]) -> Result<AnalysisResult> {
        self.analyze_with_callback(changes, &[], |_| {}).await
    }

    /// Analyze changes with in-flight changes context.
    ///
    /// The `in_flight_ids` parameter specifies changes that are currently executing.
    /// These changes are not selectable for the execution order but can be referenced
    /// as dependencies by queued changes.
    pub async fn analyze_with_inflight(
        &self,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> Result<AnalysisResult> {
        self.analyze_with_callback(changes, in_flight_ids, |_| {})
            .await
    }

    /// Analyze changes with output callback and return raw analysis result.
    ///
    /// The `in_flight_ids` parameter specifies changes that are currently executing.
    /// These changes are not selectable for the execution order but can be referenced
    /// as dependencies by queued changes.
    pub async fn analyze_with_callback<F>(
        &self,
        changes: &[Change],
        in_flight_ids: &[String],
        mut on_output: F,
    ) -> Result<AnalysisResult>
    where
        F: FnMut(String),
    {
        if changes.is_empty() {
            return Ok(AnalysisResult {
                order: Vec::new(),
                dependencies: HashMap::new(),
                groups: None,
            });
        }

        // For a single change, LLM analysis is unnecessary, but proposal
        // metadata dependencies are still authoritative hard dependencies.
        if changes.len() == 1 {
            let result = AnalysisResult {
                order: vec![changes[0].id.clone()],
                dependencies: HashMap::new(),
                groups: None,
            };
            return self.normalize_and_validate_result(result, changes, in_flight_ids);
        }

        // Build prompt for LLM analysis
        let prompt = self.build_parallelization_prompt(changes, in_flight_ids);
        info!(
            "Analyzing {} changes for parallelization (with {} in-flight)",
            changes.len(),
            in_flight_ids.len()
        );
        for c in changes {
            info!("  - {}", c.id);
        }
        if !in_flight_ids.is_empty() {
            info!("In-flight changes (executing):");
            for id in in_flight_ids {
                info!("  - {}", id);
            }
        }
        debug!("Analysis prompt: {}", prompt);

        // Start the optional non-authoritative dependency judge *beside* the
        // conventional analyzer. It runs in its own task, so it makes progress
        // concurrently and can never become a step the conventional path waits
        // for.
        let shadow = self.start_dependency_judge(changes, in_flight_ids);

        // Execute analysis command and collect output
        let command_outcome = self
            .execute_analysis_command(&prompt, changes, &mut on_output)
            .await;

        // Conventional completion is the judge's return deadline, whether the
        // conventional path produced a result or an error: a judge that has not
        // finished by now is cancelled and cleaned up by a detached task, and
        // only an already completed valid observation is retained.
        let observation = match shadow {
            Some(run) => self.collect_dependency_judge(run).await,
            None => None,
        };

        let (full_output, status) = command_outcome?;

        // Parse and validate the output
        let result =
            self.parse_and_validate_output(&full_output, &status, changes, in_flight_ids)?;

        if let Some(observation) = observation {
            self.report_shadow_dependency_comparison(&observation, &result);
        }

        info!("Analysis complete: {} changes in order", result.order.len());
        Ok(result)
    }

    /// Execute the analysis command with streaming output.
    ///
    /// Runs the AI command via AiCommandRunner, streams output to the callback,
    /// and returns the full output string and exit status.
    async fn execute_analysis_command<F>(
        &self,
        prompt: &str,
        changes: &[Change],
        on_output: &mut F,
    ) -> Result<(String, std::process::ExitStatus)>
    where
        F: FnMut(String),
    {
        // Call LLM for analysis with streaming output via AiCommandRunner
        let template = self.config.get_analyze_command()?;
        let prompt = crate::agent::append_optional_prompt(
            prompt.to_string(),
            self.config.get_analyze_append_prompt(),
        );
        let command = crate::config::OrchestratorConfig::expand_prompt(template, &prompt);
        let (mut child, mut rx) = self
            .ai_runner
            .execute_streaming_with_retry(&command, None, Some("analyze"), None)
            .await?;

        // Collect output while streaming to callback
        let mut full_output = String::new();
        while let Some(line) = rx.recv().await {
            let text = match &line {
                AiOutputLine::Stdout(s) | AiOutputLine::Stderr(s) => s.clone(),
            };
            full_output.push_str(&text);
            full_output.push('\n');
            on_output(text);
        }

        // Wait for process to complete
        let status = child.wait().await.map_err(|e| {
            let change_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
            OrchestratorError::AgentCommand(format!(
                "Analysis process failed for changes [{}]: {}",
                change_ids.join(", "),
                e
            ))
        })?;

        Ok((full_output, status))
    }

    // ── Optional shadow dependency judge ───────────────────────────────────
    //
    // Everything below is non-authoritative. It never contributes an order, a
    // dependency, a provenance, or an error to the analysis it observes.

    /// Start the configured parallel-dependency judge when one applies.
    ///
    /// Returns `None` — with a bounded redacted diagnostic where something was
    /// actually attempted — for every skip reason: no configuration, an invalid
    /// entry, LLM analysis disabled, fewer than two nodes, no permitted pair, an
    /// unreadable proposal, or an oversized request.
    fn start_dependency_judge(
        &self,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> Option<ShadowDependencyJudgeRun> {
        let judge = self.config.get_parallel_dependency_judge()?;

        // A metadata-only result is the configured authority when LLM analysis
        // is disabled, so there is nothing to shadow.
        if !self.config.use_llm_analysis() {
            return None;
        }

        // Configuration is validated at load; an analyzer handed an unvalidated
        // configuration still refuses to spawn rather than sending a request
        // whose bounds nobody checked.
        if let Err(error) = judge.validate() {
            debug!(
                purpose = PARALLEL_DEPENDENCY_PURPOSE,
                outcome = "invalid_configuration",
                "Skipping dependency judge: {error}"
            );
            return None;
        }

        let (request, pairs) =
            match self.build_dependency_judge_request(judge, changes, in_flight_ids) {
                Ok(Some(built)) => built,
                Ok(None) => return None,
                Err(category) => {
                    Self::trace_judge_failure(category, Duration::ZERO, 0);
                    return None;
                }
            };

        let serialized = match serde_json::to_string(&request) {
            Ok(serialized) => serialized,
            Err(_) => {
                Self::trace_judge_failure(JudgeFailureCategory::Io, Duration::ZERO, pairs.len());
                return None;
            }
        };
        // Checked before spawn: an oversized request is never truncated and
        // never partially judged.
        if serialized.len() > judge.max_input_bytes() {
            Self::trace_judge_failure(
                JudgeFailureCategory::InputTooLarge,
                Duration::ZERO,
                pairs.len(),
            );
            return None;
        }

        let spec = JudgeCommandSpec {
            argv: judge.command.clone(),
            working_dir: self.repo_root.clone(),
            envs: self.config.get_command_envs(),
            timeout: Duration::from_millis(judge.timeout_ms()),
            max_output_bytes: judge.max_output_bytes(),
        };

        Some(ShadowDependencyJudgeRun {
            handle: spawn_judge(spec, serialized),
            pairs,
            model: judge.model.clone(),
            yes_threshold: judge.yes_threshold(),
        })
    }

    /// Build the deterministic batched request and its trusted local pair map.
    ///
    /// `Ok(None)` means "nothing to ask" (fewer than two nodes, or no permitted
    /// pair); `Err` means the complete request is skipped.
    fn build_dependency_judge_request(
        &self,
        judge: &ParallelDependencyJudgeConfig,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> std::result::Result<Option<ShadowJudgeRequest>, JudgeFailureCategory> {
        // One deterministic node list: queued changes in input order, then the
        // explicitly supplied in-flight changes in their supplied order. Each
        // node's index is the only thing a question ID is derived from.
        let mut queued = Vec::with_capacity(changes.len());
        let mut in_flight = Vec::new();
        let mut paths: Vec<String> = Vec::new();
        let mut seen: HashSet<&str> = HashSet::new();

        for change in changes {
            if !seen.insert(change.id.as_str()) {
                continue;
            }
            let index = paths.len();
            paths.push(format!("state.queued_changes[{}]", queued.len()));
            queued.push(JudgeChangeNode {
                index,
                id: change.id.clone(),
                proposal: self.read_judge_proposal(&change.id, judge)?,
                metadata_dependencies: change.dependencies.clone(),
                priority: change
                    .metadata
                    .priority
                    .map(priority_token)
                    .map(String::from),
                references: change.metadata.references.clone(),
            });
        }
        let queued_count = queued.len();

        for id in in_flight_ids {
            if !seen.insert(id.as_str()) {
                continue;
            }
            let index = paths.len();
            paths.push(format!("state.in_flight_changes[{}]", in_flight.len()));
            in_flight.push(JudgeChangeNode {
                index,
                id: id.clone(),
                proposal: self.read_judge_proposal(id, judge)?,
                // An in-flight change is supplied to the analyzer as an ID
                // only; Conflux does not re-derive its metadata for a shadow
                // request, and its authoritative metadata is unaffected either
                // way.
                metadata_dependencies: Vec::new(),
                priority: None,
                references: Vec::new(),
            });
        }

        let node_ids: Vec<&str> = queued
            .iter()
            .chain(in_flight.iter())
            .map(|node| node.id.as_str())
            .collect();
        if node_ids.len() < 2 || queued_count == 0 {
            return Ok(None);
        }

        let mut questions = BTreeMap::new();
        let mut pairs = BTreeMap::new();
        for dependent in 0..queued_count {
            for candidate in 0..node_ids.len() {
                // The dependent is always queued; the candidate dependency is
                // any other queued or explicitly supplied in-flight change.
                if dependent == candidate {
                    continue;
                }
                let question_id = format!("q_{dependent:04}_{candidate:04}");
                let candidate_kind = if candidate < queued_count {
                    "queued"
                } else {
                    "in-flight"
                };
                questions.insert(
                    question_id.clone(),
                    NoulQuestion {
                        question_type: NOUL_QUESTION_TYPE.to_string(),
                        instructions: format!(
                            "Does queued change `{}` require repository output produced by \
                             {candidate_kind} change `{}` before it can be implemented or \
                             verified correctly?",
                            paths[dependent], paths[candidate]
                        ),
                        criteria: NoulCriteria {
                            when_true: JUDGE_TRUE_CRITERIA.to_string(),
                            when_false: JUDGE_FALSE_CRITERIA.to_string(),
                        },
                    },
                );
                pairs.insert(
                    question_id,
                    (
                        node_ids[dependent].to_string(),
                        node_ids[candidate].to_string(),
                    ),
                );
            }
        }

        if pairs.is_empty() {
            return Ok(None);
        }

        Ok(Some((
            SystemOneRequest {
                state: JudgeRequestState {
                    schema_version: JUDGE_STATE_SCHEMA_VERSION,
                    queued_changes: queued,
                    in_flight_changes: in_flight,
                },
                model: judge.model.clone(),
                questions,
            },
            pairs,
        )))
    }

    /// Read one complete `proposal.md` for the shadow request.
    ///
    /// The path is always `repo_root/openspec/changes/<validated-id>/proposal.md`.
    /// An untrusted change ID can neither escape that layout nor name a file
    /// outside it, and any absent, unreadable, non-regular, or oversized
    /// proposal skips the complete request rather than substituting `null` or
    /// judging part of the working set.
    fn read_judge_proposal(
        &self,
        change_id: &str,
        judge: &ParallelDependencyJudgeConfig,
    ) -> std::result::Result<String, JudgeFailureCategory> {
        if !is_judge_safe_change_id(change_id) {
            return Err(JudgeFailureCategory::Io);
        }

        let path = self
            .repo_root
            .join("openspec")
            .join("changes")
            .join(change_id)
            .join("proposal.md");

        let metadata = std::fs::metadata(&path).map_err(|_| JudgeFailureCategory::Io)?;
        if !metadata.is_file() {
            return Err(JudgeFailureCategory::Io);
        }
        if metadata.len() > judge.max_input_bytes() as u64 {
            return Err(JudgeFailureCategory::InputTooLarge);
        }

        std::fs::read_to_string(&path).map_err(|_| JudgeFailureCategory::Io)
    }

    /// Stop racing the judge and keep only a completed, strictly valid response.
    async fn collect_dependency_judge(
        &self,
        run: ShadowDependencyJudgeRun,
    ) -> Option<DependencyJudgeObservation> {
        let ShadowDependencyJudgeRun {
            handle,
            pairs,
            model,
            yes_threshold,
        } = run;
        let expected_ids: BTreeSet<String> = pairs.keys().cloned().collect();

        let output = match handle.collect_or_cancel().await {
            JudgeCollection::Completed(Ok(output)) => output,
            JudgeCollection::Completed(Err(failure)) => {
                Self::trace_judge_failure(failure.category, failure.duration, expected_ids.len());
                return None;
            }
            JudgeCollection::Cancelled(cleanup) => {
                // Dropping the join handle detaches the cleanup task: it sends
                // TERM, waits the fixed grace, escalates to KILL if needed, and
                // reaps the child without the authoritative path waiting for it.
                drop(cleanup);
                Self::trace_judge_failure(
                    JudgeFailureCategory::Cancelled,
                    Duration::ZERO,
                    expected_ids.len(),
                );
                return None;
            }
        };

        match parse_system_one_response(&output.stdout, &model, &expected_ids) {
            Ok(response) => {
                let mut values = HashMap::with_capacity(response.values.len());
                for (question_id, value) in response.values {
                    // The local pair map is the only parser authority: an answer
                    // ID is never trusted to describe its own change pair.
                    if let Some(pair) = pairs.get(&question_id) {
                        values.insert(pair.clone(), value);
                    }
                }
                Some(DependencyJudgeObservation {
                    model: response.model,
                    values,
                    usage: response.usage,
                    duration: output.duration,
                    yes_threshold,
                })
            }
            Err(category) => {
                Self::trace_judge_failure(category, output.duration, expected_ids.len());
                None
            }
        }
    }

    /// Compare the retained shadow observation with the authoritative result.
    ///
    /// Counting only. The normalized [`AnalysisResult`] is already final at this
    /// point and is not touched.
    fn report_shadow_dependency_comparison(
        &self,
        observation: &DependencyJudgeObservation,
        result: &AnalysisResult,
    ) {
        let authoritative: HashSet<(&str, &str)> = result
            .dependencies
            .iter()
            .flat_map(|(change_id, deps)| {
                deps.iter()
                    .map(move |dep| (change_id.as_str(), dep.as_str()))
            })
            .collect();

        let mut matching_positive = 0usize;
        let mut judge_only_positive = 0usize;
        let mut analyzer_only_positive = 0usize;
        let mut matching_negative = 0usize;

        for ((dependent, dependency), value) in &observation.values {
            let judge_yes = *value >= observation.yes_threshold;
            let analyzer_yes = authoritative.contains(&(dependent.as_str(), dependency.as_str()));
            match (judge_yes, analyzer_yes) {
                (true, true) => matching_positive += 1,
                (true, false) => judge_only_positive += 1,
                (false, true) => analyzer_only_positive += 1,
                (false, false) => matching_negative += 1,
            }
        }

        // Allowed fields only: purpose, outcome, duration, answer counts, the
        // validated model identity, and aggregate comparison counts. No request
        // state, proposal text, process output, credentials, or per-pair
        // judgment tied to a named change is ever recorded.
        info!(
            purpose = PARALLEL_DEPENDENCY_PURPOSE,
            outcome = "observed",
            duration_ms = observation.duration.as_millis() as u64,
            expected_answers = observation.values.len(),
            returned_answers = observation.values.len(),
            model = %observation.model,
            input_tokens = observation.usage.input_tokens,
            output_tokens = observation.usage.output_tokens,
            matching_positive_edges = matching_positive,
            judge_only_positive_edges = judge_only_positive,
            analyzer_only_positive_edges = analyzer_only_positive,
            matching_negative_pairs = matching_negative,
            total_pairs = observation.values.len(),
            "Shadow dependency judge observation compared with authoritative analysis"
        );
    }

    /// Emit the one bounded diagnostic a judge failure is allowed to produce.
    ///
    /// The model identity is deliberately absent: it is only trustworthy after
    /// schema validation, which by definition did not complete here.
    fn trace_judge_failure(
        category: JudgeFailureCategory,
        duration: Duration,
        expected_answers: usize,
    ) {
        warn!(
            purpose = PARALLEL_DEPENDENCY_PURPOSE,
            outcome = category.as_str(),
            duration_ms = duration.as_millis() as u64,
            expected_answers,
            "Shadow dependency judge produced no usable observation; authoritative analysis is unaffected"
        );
    }

    /// Parse and validate the analysis output.
    ///
    /// Extracts JSON from stream-json format if applicable, validates the schema,
    /// and checks the exit status. Returns the parsed AnalysisResult.
    ///
    /// The `in_flight_ids` parameter specifies changes that are currently executing.
    /// These IDs are accepted as valid dependency targets during validation even
    /// though they are not present in the `order` array.
    fn parse_and_validate_output(
        &self,
        full_output: &str,
        status: &std::process::ExitStatus,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> Result<AnalysisResult> {
        let archived_ids = self.collect_archived_change_ids();
        // Extract result from stream-json format if applicable
        let response = self.extract_stream_json_result(full_output);
        debug!("LLM response: {}", response);

        // Parse JSON response with strict validation
        let result = self
            .parse_response(&response, changes, in_flight_ids)
            .map_err(|e| {
            let preview = response.chars().take(200).collect::<String>();
            let change_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
            let err_text = e.to_string();
            if err_text.contains("Invalid dependency reference") || err_text.contains("Missing dependency reference") || err_text.contains("Rejected dependency reference") {
                let decorated = self.decorate_dependency_error_with_archive_context(
                    &err_text,
                    changes,
                    in_flight_ids,
                    &archived_ids,
                );
                OrchestratorError::Parse(format!(
                    "Analysis dependency contract failure for changes [{}] (exit code: {:?}): {}. Response preview: {}",
                    change_ids.join(", "),
                    status.code(),
                    decorated,
                    preview
                ))
            } else {
                OrchestratorError::Parse(format!(
                    "Analysis returned invalid JSON for changes [{}] (exit code: {:?}): {}. Response preview: {}",
                    change_ids.join(", "),
                    status.code(),
                    e,
                    preview
                ))
            }
        })?;

        // Check exit code after successful JSON parsing
        if !status.success() {
            let change_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
            return Err(OrchestratorError::AgentCommand(format!(
                "Analysis failed for changes [{}] with exit code: {:?}",
                change_ids.join(", "),
                status.code()
            )));
        }

        Ok(result)
    }

    /// Analyze changes and return parallel execution groups
    ///
    /// Returns groups in topological order (dependencies first).
    ///
    /// # Deprecated
    ///
    /// This method converts order-based results to group-based format.
    /// Prefer using `analyze()` for order-based execution.
    pub async fn analyze_groups(&self, changes: &[Change]) -> Result<Vec<ParallelGroup>> {
        self.analyze_groups_with_callback(changes, |_| {}).await
    }

    /// Analyze changes and return parallel execution groups with output callback
    ///
    /// The callback is called for each line of output from the analysis command.
    /// Returns groups in topological order (dependencies first).
    ///
    /// # Deprecated
    ///
    /// This method converts order-based results to group-based format.
    /// Prefer using `analyze_with_callback()` for order-based execution.
    pub async fn analyze_groups_with_callback<F>(
        &self,
        changes: &[Change],
        on_output: F,
    ) -> Result<Vec<ParallelGroup>>
    where
        F: FnMut(String),
    {
        // Use the new analyze_with_callback and convert to groups (no in-flight for deprecated method)
        let result = self.analyze_with_callback(changes, &[], on_output).await?;

        if result.order.is_empty() {
            return Ok(Vec::new());
        }

        // Convert order-based result to group-based format for backward compatibility
        let groups = self.order_to_groups(&result);
        info!("Analysis complete: {} groups identified", groups.len());
        Ok(groups)
    }

    /// Extract final assistant text from supported agent JSONL output formats.
    fn extract_stream_json_result(&self, output: &str) -> String {
        crate::stream_json_textifier::extract_final_text_from_stream_json_output(output)
    }

    /// Build the prompt for parallelization analysis
    ///
    /// Formats all changes with:
    /// - Full proposal file path for each change (e.g., `openspec/changes/{id}/proposal.md`)
    ///
    /// Also includes in-flight changes (currently executing) for dependency analysis,
    /// marked as not selectable but available as dependency targets.
    ///
    /// This makes it clear to the LLM which changes need analysis and where
    /// to find their proposal files.
    fn build_parallelization_prompt(&self, changes: &[Change], in_flight_ids: &[String]) -> String {
        let change_list: String = changes
            .iter()
            .map(|change| self.format_change_prompt_entry(change))
            .collect::<Vec<_>>()
            .join("\n\n");

        let executing_section = if in_flight_ids.is_empty() {
            String::new()
        } else {
            let executing_list: String = in_flight_ids
                .iter()
                .map(|id| format!("- {} (openspec/changes/{}/proposal.md)", id, id))
                .collect::<Vec<_>>()
                .join("\n");

            format!(
                r#"

Currently executing changes (NOT selectable, but available as dependencies):
{executing_list}
"#
            )
        };

        format!(
            r#"You are planning the execution order for OpenSpec changes.

Analyze ONLY the changes marked with [x] below.
Read the proposal files at the specified paths to understand their dependencies:

{change_list}{executing_section}

Your task:
1. Read each change's proposal.md at the given path to understand what it does
2. Use proposal frontmatter `dependencies` as the dependency source when present; only fall back to the body `## Dependencies` section when frontmatter dependencies are absent
3. Treat proposal frontmatter `priority` as a soft ordering hint for `order` only
4. Treat proposal frontmatter `references` as supplemental analysis context only
5. Consider currently executing changes as potential dependencies (but DO NOT include them in the order)
6. Return execution order and dependencies
7. `dependencies` may reference ONLY queued change IDs and explicitly listed in-flight IDs
8. NEVER reference unrelated active changes, archived changes, or any ID outside that allowed set

Return ONLY valid JSON in this exact format:
{{
  "order": ["change-a", "change-b", "change-c"],
  "dependencies": {{
    "change-c": ["change-a"]
  }}
}}

Rules:
- `order`: Array of change IDs in recommended execution sequence
  - This represents the RECOMMENDED execution order considering dependencies, priorities, and efficiency
  - Independent changes can be ordered by priority or logical flow
  - DO NOT include currently executing changes in the order (they are already running)
- `dependencies`: Object mapping change IDs to arrays of their REQUIRED dependency IDs
  - STRICT CRITERIA: Only include a dependency if one change REQUIRES the artifacts, specs, or APIs from another change to function
  - DO NOT include dependencies based on priority, references, preferred order, or efficiency alone
  - Example of REQUIRED dependency: "change-b implements a feature using the API defined in change-a"
  - Example of NOT a dependency: "change-a should ideally be done before change-b for efficiency"
  - Dependencies CAN reference currently executing changes if a queued change requires their output
- Dependencies MUST reference only IDs from the queued `order` set and explicitly provided in-flight IDs
- Do NOT reference active-but-not-queued IDs, archived IDs, or unrelated change IDs
- Proposal metadata warnings are informational only; continue analysis using known metadata
- Every change ID in the input list must appear exactly once in `order`
- Dependencies are hard constraints: a change CANNOT start until all its dependencies are merged to base
- Order preferences without required dependencies should be reflected in `order` only, not in `dependencies`
- Return valid JSON only, no markdown, no explanation"#
        )
    }

    fn format_change_prompt_entry(&self, change: &Change) -> String {
        let proposal_path = format!("openspec/changes/{}/proposal.md", change.id);
        let metadata = self.read_prompt_metadata(change);

        let mut lines = vec![format!("- {} ({})", change.id, proposal_path)];
        lines.push(format!(
            "  dependency_source: {}",
            if change.dependencies.is_empty() {
                "none"
            } else {
                "proposal metadata or body fallback"
            }
        ));
        lines.push(format!(
            "  dependencies_for_analysis: {}",
            if change.dependencies.is_empty() {
                "[]".to_string()
            } else {
                format!("[{}]", change.dependencies.join(", "))
            }
        ));

        if let Some(metadata) = metadata {
            if let Some(priority) = metadata.priority {
                lines.push(format!("  priority_hint: {}", priority));
            }
            if !metadata.references.is_empty() {
                lines.push(format!(
                    "  references: [{}]",
                    metadata.references.join(", ")
                ));
            }
            if !metadata.warnings.is_empty() {
                lines.push(format!(
                    "  metadata_warnings: [{}]",
                    metadata.warnings.join(" | ")
                ));
            }
        }

        lines.join("\n")
    }

    fn read_prompt_metadata(&self, change: &Change) -> Option<AnalyzePromptMetadata> {
        let proposal = crate::openspec::read_proposal(&change.id);
        let metadata = proposal.metadata?;

        for warning_message in metadata
            .warnings
            .iter()
            .map(|warning| warning.message.as_str())
        {
            warn!(
                change_id = %change.id,
                warning = warning_message,
                "Continuing analyze with proposal frontmatter warning"
            );
        }

        Some(AnalyzePromptMetadata::from_frontmatter(&metadata))
    }

    /// Parse LLM response into AnalysisResult
    ///
    /// The `in_flight_ids` parameter specifies currently executing change IDs that
    /// are valid dependency targets but not expected in `order`.
    fn parse_response(
        &self,
        response: &str,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> Result<AnalysisResult> {
        // Try to extract JSON from response (may have surrounding text)
        let json_str = self.extract_json(response)?;

        // Strict JSON schema validation
        self.validate_json_schema(&json_str)?;

        // Parse JSON into structured type
        let result: AnalysisResult = serde_json::from_str(&json_str).map_err(|e| {
            OrchestratorError::Parse(format!("Failed to parse parallelization response: {}", e))
        })?;

        // Validate all change IDs exist
        self.validate_change_ids(&result, changes)?;

        self.normalize_and_validate_result(result, changes, in_flight_ids)
    }

    fn normalize_and_validate_result(
        &self,
        mut result: AnalysisResult,
        changes: &[Change],
        in_flight_ids: &[String],
    ) -> Result<AnalysisResult> {
        for change in changes {
            union_metadata_dependencies(&mut result.dependencies, &change.id, &change.dependencies);
        }

        let archived_ids = self.collect_archived_change_ids();
        let rejected_ids = self.collect_rejected_change_ids();
        self.validate_dependency_graph_with_changes(
            &result,
            changes,
            in_flight_ids,
            &archived_ids,
            &rejected_ids,
        )?;

        Ok(result)
    }

    /// Validate JSON schema for analysis result.
    ///
    /// Checks that:
    /// - JSON is parseable
    /// - `order` key exists
    /// - `order` is an array
    /// - `dependencies` key exists (optional but should be present)
    fn validate_json_schema(&self, json_str: &str) -> Result<()> {
        // Parse as generic JSON value first
        let value: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| OrchestratorError::Parse(format!("Invalid JSON syntax: {}", e)))?;

        // Check for required root object
        if !value.is_object() {
            return Err(OrchestratorError::Parse(
                "JSON root must be an object".to_string(),
            ));
        }

        // Check for required `order` key
        let order = value.get("order").ok_or_else(|| {
            OrchestratorError::Parse("Missing required key 'order' in JSON".to_string())
        })?;

        // Check that order is an array
        if !order.is_array() {
            return Err(OrchestratorError::Parse(
                "Key 'order' must be an array".to_string(),
            ));
        }

        // Check for dependencies key (should be present)
        if let Some(dependencies) = value.get("dependencies") {
            if !dependencies.is_object() {
                return Err(OrchestratorError::Parse(
                    "Key 'dependencies' must be an object".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Extract JSON from response text (handles markdown code blocks)
    fn extract_json(&self, response: &str) -> Result<String> {
        let trimmed = response.trim();
        let candidate = if let Some(start) = trimmed.find("```json") {
            let after_marker = &trimmed[start + 7..];
            after_marker
                .find("```")
                .map(|end| after_marker[..end].trim())
        } else if let Some(start) = trimmed.find("```") {
            let after_marker = &trimmed[start + 3..];
            let content_start = after_marker.find('\n').unwrap_or(0);
            let content = &after_marker[content_start..];
            content.find("```").map(|end| content[..end].trim())
        } else {
            trimmed.find('{').map(|start| &trimmed[start..])
        }
        .ok_or_else(|| {
            OrchestratorError::Parse("Could not extract JSON from response".to_string())
        })?;

        let value = serde_json::Deserializer::from_str(candidate)
            .into_iter::<serde_json::Value>()
            .next()
            .ok_or_else(|| {
                OrchestratorError::Parse("Could not extract JSON from response".to_string())
            })?
            .map_err(|e| OrchestratorError::Parse(format!("Invalid JSON syntax: {}", e)))?;

        serde_json::to_string(&value).map_err(|e| {
            OrchestratorError::Parse(format!("Failed to normalize JSON response: {}", e))
        })
    }

    /// Validate that all change IDs in the result exist in the input
    fn validate_change_ids(&self, result: &AnalysisResult, changes: &[Change]) -> Result<()> {
        let valid_ids: HashSet<&str> = changes.iter().map(|c| c.id.as_str()).collect();
        let mut seen_ids: HashSet<&str> = HashSet::new();

        // Check all IDs in order
        for change_id in &result.order {
            // Check ID exists
            if !valid_ids.contains(change_id.as_str()) {
                return Err(OrchestratorError::Parse(format!(
                    "Unknown change ID in order: {}",
                    change_id
                )));
            }

            // Check for duplicates
            if seen_ids.contains(change_id.as_str()) {
                return Err(OrchestratorError::Parse(format!(
                    "Duplicate change ID in order: {}",
                    change_id
                )));
            }
            seen_ids.insert(change_id.as_str());
        }

        // Check all changes are accounted for
        if seen_ids.len() != valid_ids.len() {
            let missing: Vec<_> = valid_ids.difference(&seen_ids).collect();
            return Err(OrchestratorError::Parse(format!(
                "Missing change IDs in response: {:?}",
                missing
            )));
        }

        Ok(())
    }

    /// Validate dependency graph for circular dependencies
    ///
    /// The `in_flight_ids` parameter specifies currently executing change IDs.
    /// Dependencies referencing these IDs are considered valid even though
    /// they are not present in the `order` array.
    #[cfg_attr(not(test), allow(dead_code))]
    fn validate_dependency_graph(
        &self,
        result: &AnalysisResult,
        in_flight_ids: &[String],
    ) -> Result<()> {
        let in_flight_set: HashSet<&str> = in_flight_ids.iter().map(|s| s.as_str()).collect();

        // Check for self-dependencies
        for (change_id, deps) in &result.dependencies {
            if deps.contains(change_id) {
                return Err(OrchestratorError::Parse(format!(
                    "Self-dependency detected: change '{}' depends on itself",
                    change_id
                )));
            }

            // Check all dependencies exist in order or in-flight
            for dep_id in deps {
                if !result.order.contains(dep_id) && !in_flight_set.contains(dep_id.as_str()) {
                    let mut allowed_ids: Vec<String> = result.order.clone();
                    allowed_ids.extend(in_flight_ids.iter().cloned());
                    allowed_ids.sort();
                    allowed_ids.dedup();

                    return Err(OrchestratorError::Parse(format!(
                        "Invalid dependency reference: change '{}' depends on '{}' outside allowed dependency targets. allowed_queued_ids={:?}, allowed_in_flight_ids={:?}, allowed_ids={:?}",
                        change_id,
                        dep_id,
                        result.order,
                        in_flight_ids,
                        allowed_ids
                    )));
                }
            }
        }

        // Check for circular dependencies using DFS
        self.detect_cycles_from_dependencies(&result.dependencies)?;

        Ok(())
    }

    fn validate_dependency_graph_with_changes(
        &self,
        result: &AnalysisResult,
        changes: &[Change],
        in_flight_ids: &[String],
        archived_ids: &HashSet<String>,
        rejected_ids: &HashSet<String>,
    ) -> Result<()> {
        let queued_ids: Vec<&str> = changes.iter().map(|change| change.id.as_str()).collect();
        let in_flight_refs: Vec<&str> = in_flight_ids.iter().map(String::as_str).collect();
        let active_ids = self.collect_active_change_ids();
        let active_refs: Vec<&str> = active_ids.iter().map(String::as_str).collect();

        for (change_id, deps) in &result.dependencies {
            if deps.contains(change_id) {
                return Err(OrchestratorError::Parse(format!(
                    "Self-dependency detected: change '{}' depends on itself",
                    change_id
                )));
            }

            for dep_id in deps {
                match classify_dependency_target(
                    dep_id,
                    queued_ids.iter().copied(),
                    in_flight_refs.iter().copied(),
                    active_refs.iter().copied(),
                    archived_ids,
                    rejected_ids,
                ) {
                    DependencyTargetClass::Queued
                    | DependencyTargetClass::InFlight
                    | DependencyTargetClass::Resolving
                    | DependencyTargetClass::ActiveButNotQueued => {}
                    DependencyTargetClass::Error => unreachable!(
                        "repository-visible dependency classification cannot produce terminal-error state"
                    ),
                    DependencyTargetClass::Archived => {
                        debug!(
                            change_id,
                            dependency = dep_id,
                            "Accepted archived dependency target as already satisfied"
                        );
                    }
                    DependencyTargetClass::Rejected => {
                        let mut allowed_ids: Vec<String> = result.order.clone();
                        allowed_ids.extend(in_flight_ids.iter().cloned());
                        allowed_ids.extend(active_ids.iter().cloned());
                        allowed_ids.extend(archived_ids.iter().cloned());
                        allowed_ids.extend(rejected_ids.iter().cloned());
                        allowed_ids.sort();
                        allowed_ids.dedup();

                        return Err(OrchestratorError::Parse(format!(
                            "Rejected dependency reference: change '{}' depends on '{}' classified as rejected dependency target. allowed_queued_ids={:?}, allowed_in_flight_ids={:?}, allowed_active_ids={:?}, allowed_archived_ids={:?}, allowed_rejected_ids={:?}, allowed_ids={:?}",
                            change_id,
                            dep_id,
                            result.order,
                            in_flight_ids,
                            active_ids,
                            archived_ids,
                            rejected_ids,
                            allowed_ids
                        )));
                    }
                    DependencyTargetClass::Missing => {
                        let mut allowed_ids: Vec<String> = result.order.clone();
                        allowed_ids.extend(in_flight_ids.iter().cloned());
                        allowed_ids.extend(active_ids.iter().cloned());
                        allowed_ids.extend(archived_ids.iter().cloned());
                        allowed_ids.extend(rejected_ids.iter().cloned());
                        allowed_ids.sort();
                        allowed_ids.dedup();

                        return Err(OrchestratorError::Parse(format!(
                            "Missing dependency reference: change '{}' depends on '{}' classified as missing dependency target. allowed_queued_ids={:?}, allowed_in_flight_ids={:?}, allowed_active_ids={:?}, allowed_archived_ids={:?}, allowed_rejected_ids={:?}, allowed_ids={:?}",
                            change_id,
                            dep_id,
                            result.order,
                            in_flight_ids,
                            active_ids,
                            archived_ids,
                            rejected_ids,
                            allowed_ids
                        )));
                    }
                }
            }
        }

        self.detect_cycles_from_dependencies(&result.dependencies)?;

        Ok(())
    }

    fn collect_active_change_ids(&self) -> HashSet<String> {
        collect_active_change_ids(Path::new("."))
    }

    fn collect_archived_change_ids(&self) -> HashSet<String> {
        collect_archived_change_ids(Path::new("."))
    }

    fn collect_rejected_change_ids(&self) -> HashSet<String> {
        collect_rejected_change_ids(Path::new("."))
    }

    fn decorate_dependency_error_with_archive_context(
        &self,
        err_text: &str,
        changes: &[Change],
        in_flight_ids: &[String],
        archived_ids: &HashSet<String>,
    ) -> String {
        static DEP_RE: OnceLock<Regex> = OnceLock::new();
        let dep_re = DEP_RE.get_or_init(|| {
            Regex::new(r"change '([^']+)' depends on '([^']+)'(?: outside allowed dependency targets| classified as (?:missing|rejected) dependency target)")
                .unwrap()
        });

        let Some(caps) = dep_re.captures(err_text) else {
            return err_text.to_string();
        };

        let change_id = caps.get(1).map_or("", |m| m.as_str());
        let dep_id = caps.get(2).map_or("", |m| m.as_str());
        if change_id.is_empty() || dep_id.is_empty() {
            return err_text.to_string();
        }

        let queued_ids: Vec<&str> = changes.iter().map(|c| c.id.as_str()).collect();
        let in_flight_set: HashSet<&str> = in_flight_ids.iter().map(|id| id.as_str()).collect();
        let active_ids = self.collect_active_change_ids();
        let active_refs: Vec<&str> = active_ids.iter().map(String::as_str).collect();

        let rejected_ids = self.collect_rejected_change_ids();
        let classification = classify_dependency_target(
            dep_id,
            queued_ids.iter().copied(),
            in_flight_set.iter().copied(),
            active_refs.iter().copied(),
            archived_ids,
            &rejected_ids,
        );

        format!(
            "{} dependency_target_classification={{change:'{}', dependency:'{}', class:'{}'}}",
            err_text,
            change_id,
            dep_id,
            classification.as_str()
        )
    }

    /// Detect cycles in dependency graph (change-level dependencies)
    fn detect_cycles_from_dependencies(
        &self,
        dependencies: &HashMap<String, Vec<String>>,
    ) -> Result<()> {
        let mut visited: HashSet<String> = HashSet::new();
        let mut rec_stack: HashSet<String> = HashSet::new();

        for change_id in dependencies.keys() {
            if !visited.contains(change_id)
                && self.has_cycle_in_dependencies(
                    change_id,
                    dependencies,
                    &mut visited,
                    &mut rec_stack,
                )
            {
                return Err(OrchestratorError::Parse(
                    "Circular dependency detected in change dependencies".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// DFS helper for cycle detection in change-level dependencies
    fn has_cycle_in_dependencies(
        &self,
        node: &str,
        dependencies: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
    ) -> bool {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());

        if let Some(deps) = dependencies.get(node) {
            for dep in deps {
                if !visited.contains(dep) {
                    if self.has_cycle_in_dependencies(dep, dependencies, visited, rec_stack) {
                        return true;
                    }
                } else if rec_stack.contains(dep) {
                    return true;
                }
            }
        }

        rec_stack.remove(node);
        false
    }

    /// Convert order-based analysis result to group-based format.
    ///
    /// Groups changes that have no dependencies on each other into parallel groups.
    /// Changes are processed in the order specified by the `order` field, respecting
    /// the constraints defined in the `dependencies` field.
    fn order_to_groups(&self, result: &AnalysisResult) -> Vec<ParallelGroup> {
        let mut groups: Vec<ParallelGroup> = Vec::new();
        let mut processed: HashSet<String> = HashSet::new();
        let mut group_id = 1u32;

        // Process changes in order
        for change_id in &result.order {
            if processed.contains(change_id) {
                continue;
            }

            // Find all changes that can be executed in parallel with this one
            let mut group_changes = vec![change_id.clone()];
            processed.insert(change_id.clone());

            // Check remaining unprocessed changes
            for other_id in &result.order {
                if processed.contains(other_id) {
                    continue;
                }

                // Can run in parallel if:
                // 1. No dependency between them
                // 2. All dependencies are already processed
                let can_parallel =
                    !self.has_dependency_between(change_id, other_id, &result.dependencies)
                        && self.dependencies_satisfied(other_id, &result.dependencies, &processed);

                if can_parallel {
                    group_changes.push(other_id.clone());
                    processed.insert(other_id.clone());
                }
            }

            // Create a group for these parallel changes
            groups.push(ParallelGroup {
                id: group_id,
                changes: group_changes,
                depends_on: Vec::new(), // Dependencies are tracked at change level
            });
            group_id += 1;
        }

        groups
    }

    /// Check if there's a dependency relationship between two changes (in either direction)
    fn has_dependency_between(
        &self,
        a: &str,
        b: &str,
        dependencies: &HashMap<String, Vec<String>>,
    ) -> bool {
        // Check if a depends on b
        if let Some(a_deps) = dependencies.get(a) {
            if a_deps.contains(&b.to_string()) {
                return true;
            }
        }
        // Check if b depends on a
        if let Some(b_deps) = dependencies.get(b) {
            if b_deps.contains(&a.to_string()) {
                return true;
            }
        }
        false
    }

    /// Check if all dependencies for a change are satisfied (already processed)
    fn dependencies_satisfied(
        &self,
        change_id: &str,
        dependencies: &HashMap<String, Vec<String>>,
        processed: &HashSet<String>,
    ) -> bool {
        if let Some(deps) = dependencies.get(change_id) {
            deps.iter().all(|dep| processed.contains(dep))
        } else {
            true // No dependencies
        }
    }
}

/// Extract change-level dependencies from parallel groups.
///
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_command_runner::{AiCommandRunner, SharedStaggerState};
    use crate::command_queue::CommandQueueConfig;
    use crate::config::defaults::*;
    use crate::openspec::ProposalMetadata;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn create_test_change(id: &str) -> Change {
        Change {
            id: id.to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: crate::openspec::ProposalMetadata::default(),
        }
    }

    fn create_test_analyzer() -> ParallelizationAnalyzer {
        create_test_analyzer_with(
            crate::config::OrchestratorConfig::default(),
            PathBuf::from("."),
        )
    }

    fn create_test_analyzer_with(
        config: crate::config::OrchestratorConfig,
        repo_root: PathBuf,
    ) -> ParallelizationAnalyzer {
        let shared_stagger_state: SharedStaggerState = Arc::new(Mutex::new(None));
        let queue_config = CommandQueueConfig {
            acceptance_max_runtime_secs:
                crate::config::defaults::DEFAULT_ACCEPTANCE_MAX_RUNTIME_SECS,
            stagger_delay_ms: config
                .command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config
                .command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config
                .command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config
                .command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config
                .command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
            inactivity_timeout_secs: config.get_command_inactivity_timeout_secs(),
            inactivity_kill_grace_secs: config.get_command_inactivity_kill_grace_secs(),
            inactivity_timeout_max_retries: config.get_command_inactivity_timeout_max_retries(),
            strict_process_cleanup: config.get_command_strict_process_cleanup(),
            max_runtime_secs: config.get_command_max_runtime_secs(),
        };
        let stream_json_textify = config.get_stream_json_textify();
        let mut ai_runner = AiCommandRunner::new(queue_config, shared_stagger_state);
        ai_runner.set_stream_json_textify(stream_json_textify);
        ai_runner.set_strict_process_cleanup(config.get_command_strict_process_cleanup());
        ParallelizationAnalyzer::new(ai_runner, config, repo_root)
    }

    #[test]
    fn test_extract_json_pure() {
        let analyzer = create_test_analyzer();

        let json = r#"{"order": ["a"], "dependencies": {}}"#;
        let result = analyzer.extract_json(json);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_json_markdown() {
        let analyzer = create_test_analyzer();

        let response = r#"Here's the analysis:

```json
{"order": ["a"], "dependencies": {}}
```

That's all."#;
        let result = analyzer.extract_json(response);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_json_ignores_trailing_agent_output() {
        let analyzer = create_test_analyzer();
        let json = r#"{"order":["a"],"dependencies":{}}"#;
        let response = format!("{json}{json}[tool_use:Read] filePath=/tmp/proposal.md");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&analyzer.extract_json(&response).unwrap())
                .unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn test_extract_json_rejects_malformed_first_object() {
        let analyzer = create_test_analyzer();
        let response = r#"{"order":["a"],"dependencies":{} {"valid":true}"#;

        assert!(analyzer.extract_json(response).is_err());
    }

    #[test]
    fn test_validate_change_ids_missing() {
        let analyzer = create_test_analyzer();

        let changes = vec![create_test_change("a"), create_test_change("b")];
        let result = AnalysisResult {
            order: vec!["a".to_string()], // Missing "b"
            dependencies: HashMap::new(),
            groups: None,
        };

        let validation = analyzer.validate_change_ids(&result, &changes);
        assert!(validation.is_err());
    }

    #[test]
    fn test_validate_change_ids_duplicate() {
        let analyzer = create_test_analyzer();

        let changes = vec![create_test_change("a"), create_test_change("b")];
        let result = AnalysisResult {
            order: vec!["a".to_string(), "a".to_string(), "b".to_string()], // Duplicate "a"
            dependencies: HashMap::new(),
            groups: None,
        };

        let validation = analyzer.validate_change_ids(&result, &changes);
        assert!(validation.is_err());
    }

    #[test]
    fn test_validate_dependency_graph_valid() {
        let analyzer = create_test_analyzer();

        let mut deps = HashMap::new();
        deps.insert("b".to_string(), vec!["a".to_string()]);
        let result = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps,
            groups: None,
        };

        let validation = analyzer.validate_dependency_graph(&result, &[]);
        assert!(validation.is_ok());
    }

    #[test]
    fn test_validate_dependency_graph_self_reference() {
        let analyzer = create_test_analyzer();

        let mut deps = HashMap::new();
        deps.insert("a".to_string(), vec!["a".to_string()]); // Self-reference
        let result = AnalysisResult {
            order: vec!["a".to_string()],
            dependencies: deps,
            groups: None,
        };

        let validation = analyzer.validate_dependency_graph(&result, &[]);
        assert!(validation.is_err());
    }

    #[test]
    fn test_validate_dependency_graph_cycle() {
        let analyzer = create_test_analyzer();

        let mut deps = HashMap::new();
        deps.insert("a".to_string(), vec!["b".to_string()]); // Cycle: a -> b -> a
        deps.insert("b".to_string(), vec!["a".to_string()]);
        let result = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps,
            groups: None,
        };

        let validation = analyzer.validate_dependency_graph(&result, &[]);
        assert!(validation.is_err());
    }

    #[test]
    fn test_build_prompt_includes_frontmatter_metadata_context() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let change_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("change-a");
        std::fs::create_dir_all(&change_dir).unwrap();
        std::fs::write(
            change_dir.join("proposal.md"),
            "---\npriority: high\ndependencies:\n  - base-change\nreferences:\n  - src/analyzer.rs\nowner: tumf\n---\n# Change: Sample\n\n## Dependencies\n\n- legacy-dep\n",
        )
        .unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let metadata =
            crate::openspec::parse_proposal_metadata_from_file(&change_dir.join("proposal.md"));
        let changes = vec![Change {
            id: "change-a".to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: metadata.dependencies.clone(),
            metadata,
        }];

        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        std::env::set_current_dir(original_dir).unwrap();

        assert!(prompt.contains("priority_hint: high"));
        assert!(prompt.contains("dependencies_for_analysis: [base-change]"));
        assert!(prompt.contains("references: [src/analyzer.rs]"));
        assert!(prompt.contains("metadata_warnings: [Unknown proposal frontmatter key: owner]"));
        assert!(prompt.contains(
            "Use proposal frontmatter `dependencies` as the dependency source when present"
        ));
        assert!(prompt.contains("Treat proposal frontmatter `priority` as a soft ordering hint"));
        assert!(prompt.contains(
            "Treat proposal frontmatter `references` as supplemental analysis context only"
        ));
    }

    #[test]
    fn test_build_prompt_with_selected_markers() {
        let analyzer = create_test_analyzer();

        let changes = vec![
            Change {
                id: "selected-a".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "unselected-b".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "selected-c".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
        ];

        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        // All changes should be included with proposal.md path (no approval markers)
        assert!(prompt.contains("- selected-a (openspec/changes/selected-a/proposal.md)"));
        assert!(prompt.contains("- selected-c (openspec/changes/selected-c/proposal.md)"));
        assert!(prompt.contains("- unselected-b (openspec/changes/unselected-b/proposal.md)"));

        // Check that instruction mentions reading proposal files
        assert!(prompt.contains("Read the proposal files at the specified paths"));
    }

    #[test]
    fn test_build_prompt_all_selected() {
        let analyzer = create_test_analyzer();

        let changes = vec![
            Change {
                id: "change-1".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "change-2".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
        ];

        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        // All should be included with proposal.md path (no approval markers)
        assert!(prompt.contains("- change-1 (openspec/changes/change-1/proposal.md)"));
        assert!(prompt.contains("- change-2 (openspec/changes/change-2/proposal.md)"));
    }

    #[test]
    fn test_build_prompt_none_selected() {
        let analyzer = create_test_analyzer();

        let changes = vec![Change {
            id: "change-1".to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }];

        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        // All changes should be included with proposal.md path (no approval markers)
        assert!(prompt.contains("- change-1 (openspec/changes/change-1/proposal.md)"));

        // Check that prompt structure is still there
        assert!(prompt.contains("Analyze ONLY the changes marked with [x]"));
    }

    #[test]
    fn test_prompt_clarifies_dependency_vs_order() {
        let analyzer = create_test_analyzer();

        let changes = vec![create_test_change("a")];
        let prompt = analyzer.build_parallelization_prompt(&changes, &[]);

        // Verify prompt clarifies that dependencies are REQUIRED relationships
        assert!(prompt.contains("REQUIRED"));
        assert!(prompt.contains("artifacts, specs, or APIs"));

        // Verify prompt explains order is for recommended sequence
        assert!(prompt.contains("recommended execution"));

        // Verify prompt warns against confusing priority with dependency
        assert!(prompt.contains("DO NOT include dependencies based on priority"));
    }

    #[test]
    fn test_validate_dependency_strict_criteria() {
        let analyzer = create_test_analyzer();

        // Valid case: b requires a's API
        let mut deps_valid = HashMap::new();
        deps_valid.insert("b".to_string(), vec!["a".to_string()]);

        let result_valid = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps_valid,
            groups: None,
        };

        // Should pass validation (dependency graph is valid)
        assert!(analyzer
            .validate_dependency_graph(&result_valid, &[])
            .is_ok());

        // Invalid case: self-dependency (still caught)
        let mut deps_invalid = HashMap::new();
        deps_invalid.insert("a".to_string(), vec!["a".to_string()]);

        let result_invalid = AnalysisResult {
            order: vec!["a".to_string()],
            dependencies: deps_invalid,
            groups: None,
        };

        // Should fail validation (self-dependency)
        assert!(analyzer
            .validate_dependency_graph(&result_invalid, &[])
            .is_err());
    }

    #[test]
    fn test_order_can_differ_from_dependency_graph() {
        let analyzer = create_test_analyzer();

        // Case: a and b are independent, but order suggests b before a (by priority)
        let result = AnalysisResult {
            order: vec!["b".to_string(), "a".to_string(), "c".to_string()],
            dependencies: HashMap::new(), // No dependencies!
            groups: None,
        };

        let changes = vec![
            create_test_change("a"),
            create_test_change("b"),
            create_test_change("c"),
        ];

        // Should validate successfully - order is just a recommendation
        assert!(analyzer.validate_change_ids(&result, &changes).is_ok());
        assert!(analyzer.validate_dependency_graph(&result, &[]).is_ok());
    }

    #[test]
    fn test_build_prompt_with_inflight_changes() {
        let analyzer = create_test_analyzer();

        let queued_changes = vec![
            Change {
                id: "queued-a".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
            Change {
                id: "queued-b".to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: Vec::new(),
                metadata: ProposalMetadata::default(),
            },
        ];

        let in_flight_ids = vec!["inflight-1".to_string(), "inflight-2".to_string()];

        let prompt = analyzer.build_parallelization_prompt(&queued_changes, &in_flight_ids);

        // Verify queued changes are in the main list (no approval markers)
        assert!(prompt.contains("- queued-a (openspec/changes/queued-a/proposal.md)"));
        assert!(prompt.contains("- queued-b (openspec/changes/queued-b/proposal.md)"));

        // Verify in-flight changes are in the executing section
        assert!(prompt.contains("Currently executing changes"));
        assert!(prompt.contains("- inflight-1 (openspec/changes/inflight-1/proposal.md)"));
        assert!(prompt.contains("- inflight-2 (openspec/changes/inflight-2/proposal.md)"));

        // Verify instructions about in-flight changes
        assert!(prompt.contains("NOT selectable"));
        assert!(prompt.contains("DO NOT include currently executing changes in the order"));
        assert!(prompt.contains("Dependencies CAN reference currently executing changes"));
        assert!(prompt.contains("`dependencies` may reference ONLY queued change IDs and explicitly listed in-flight IDs"));
        assert!(prompt.contains("NEVER reference unrelated active changes, archived changes"));
    }

    #[test]
    fn test_validate_dependency_graph_with_inflight() {
        let analyzer = create_test_analyzer();

        // Dependency on an in-flight change (not in order) should be valid
        let mut deps = HashMap::new();
        deps.insert("b".to_string(), vec!["inflight-x".to_string()]);
        let result = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps,
            groups: None,
        };

        let in_flight_ids = vec!["inflight-x".to_string()];
        let validation = analyzer.validate_dependency_graph(&result, &in_flight_ids);
        assert!(
            validation.is_ok(),
            "Dependency on in-flight change should be valid"
        );
    }

    #[test]
    fn test_validate_dependency_graph_invalid_inflight_ref() {
        let analyzer = create_test_analyzer();

        // Dependency on an ID that is neither in order nor in-flight should fail
        let mut deps = HashMap::new();
        deps.insert("b".to_string(), vec!["nonexistent".to_string()]);
        let result = AnalysisResult {
            order: vec!["a".to_string(), "b".to_string()],
            dependencies: deps,
            groups: None,
        };

        let in_flight_ids = vec!["inflight-x".to_string()];
        let validation = analyzer.validate_dependency_graph(&result, &in_flight_ids);
        assert!(
            validation.is_err(),
            "Dependency on unknown ID should be rejected"
        );

        let error_text = validation.err().unwrap().to_string();
        assert!(error_text.contains("Invalid dependency reference"));
        assert!(error_text.contains("nonexistent"));
        assert!(error_text.contains("allowed_queued_ids"));
        assert!(error_text.contains("allowed_in_flight_ids"));
    }

    #[test]
    fn test_decorate_dependency_error_classifies_archived_dependency() {
        let analyzer = create_test_analyzer();
        let changes = vec![create_test_change("change-a")];
        let in_flight_ids = vec!["inflight-x".to_string()];
        let archived_ids = HashSet::from(["archived-x".to_string()]);
        let err = "Invalid dependency reference: change 'change-a' depends on 'archived-x' outside allowed dependency targets";

        let decorated = analyzer.decorate_dependency_error_with_archive_context(
            err,
            &changes,
            &in_flight_ids,
            &archived_ids,
        );

        assert!(decorated.contains("dependency_target_classification"));
        assert!(decorated.contains("class:'archived'"));
    }

    #[test]
    fn test_collect_archived_change_ids_strips_date_prefix() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let archived_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("archive")
            .join("2026-04-29-sample-change");
        std::fs::create_dir_all(&archived_dir).unwrap();
        std::fs::write(archived_dir.join("proposal.md"), "# Archived").unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        let archived_ids = analyzer.collect_archived_change_ids();
        std::env::set_current_dir(original_dir).unwrap();

        assert!(archived_ids.contains("sample-change"));
    }

    #[tokio::test]
    async fn test_single_change_fast_path_preserves_metadata_dependency() {
        let analyzer = create_test_analyzer();
        let mut change = create_test_change("route");
        change.dependencies = vec!["policy".to_string()];

        let result = analyzer
            .analyze_with_callback(&[change], &["policy".to_string()], |_| {})
            .await
            .expect("single change analysis should succeed");

        assert_eq!(result.order, vec!["route".to_string()]);
        assert_eq!(
            result.dependencies.get("route"),
            Some(&vec!["policy".to_string()])
        );
    }

    #[test]
    fn test_parse_response_unions_metadata_dependency_omitted_by_llm() {
        let analyzer = create_test_analyzer();
        let mut route = create_test_change("route");
        route.dependencies = vec!["policy".to_string()];
        let policy = create_test_change("policy");
        let changes = vec![route, policy];
        let response = r#"{"order":["policy","route"],"dependencies":{}}"#;

        let result = analyzer
            .parse_response(response, &changes, &[])
            .expect("metadata dependency should be unioned into parsed result");

        assert_eq!(
            result.dependencies.get("route"),
            Some(&vec!["policy".to_string()])
        );
    }

    #[test]
    fn test_validate_dependency_graph_accepts_active_but_not_queued_dependency() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let policy_dir = temp_dir.path().join("openspec/changes/policy");
        std::fs::create_dir_all(&policy_dir).unwrap();
        std::fs::write(policy_dir.join("proposal.md"), "# Policy").unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut route = create_test_change("route");
        route.dependencies = vec!["policy".to_string()];
        let result =
            analyzer.parse_response(r#"{"order":["route"],"dependencies":{}}"#, &[route], &[]);

        std::env::set_current_dir(original_dir).unwrap();
        assert!(
            result.is_ok(),
            "active-but-not-queued metadata dependency should be retained for scheduler gating"
        );
        assert_eq!(
            result.unwrap().dependencies.get("route"),
            Some(&vec!["policy".to_string()])
        );
    }

    #[test]
    fn test_validate_dependency_graph_accepts_archived_dependency() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let archived_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("archive")
            .join("2026-04-29-contracts");
        std::fs::create_dir_all(&archived_dir).unwrap();
        std::fs::write(archived_dir.join("proposal.md"), "# Archived").unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut route = create_test_change("route");
        route.dependencies = vec!["contracts".to_string()];
        let result =
            analyzer.parse_response(r#"{"order":["route"],"dependencies":{}}"#, &[route], &[]);

        std::env::set_current_dir(original_dir).unwrap();
        assert!(result.is_ok(), "archived dependency should be accepted");
    }

    #[test]
    fn test_validate_dependency_graph_reports_rejected_dependency() {
        let analyzer = create_test_analyzer();
        let _lock = crate::test_support::cwd_lock().lock().unwrap();
        let temp_dir = tempfile::TempDir::new().unwrap();
        let rejected_dir = temp_dir
            .path()
            .join("openspec")
            .join("changes")
            .join("contracts");
        std::fs::create_dir_all(&rejected_dir).unwrap();
        std::fs::write(rejected_dir.join("proposal.md"), "# Rejected").unwrap();
        std::fs::write(rejected_dir.join("REJECTED.md"), "# REJECTED").unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut route = create_test_change("route");
        route.dependencies = vec!["contracts".to_string()];
        let error = analyzer
            .parse_response(r#"{"order":["route"],"dependencies":{}}"#, &[route], &[])
            .expect_err("rejected dependency should fail closed")
            .to_string();

        std::env::set_current_dir(original_dir).unwrap();
        assert!(error.contains("Rejected dependency reference"));
        assert!(error.contains("classified as rejected dependency target"));
    }

    #[test]
    fn test_validate_dependency_graph_reports_missing_dependency() {
        let analyzer = create_test_analyzer();
        let mut route = create_test_change("route");
        route.dependencies = vec!["ghost".to_string()];

        let error = analyzer
            .parse_response(r#"{"order":["route"],"dependencies":{}}"#, &[route], &[])
            .expect_err("missing dependency should fail closed")
            .to_string();

        assert!(error.contains("Missing dependency reference"));
        assert!(error.contains("classified as missing dependency target"));
        assert!(!error.contains("Analysis returned invalid JSON"));
    }

    #[test]
    fn test_build_prompt_without_inflight_changes() {
        let analyzer = create_test_analyzer();

        let queued_changes = vec![Change {
            id: "queued-a".to_string(),
            completed_tasks: 0,
            total_tasks: 5,
            last_modified: "now".to_string(),
            dependencies: Vec::new(),
            metadata: ProposalMetadata::default(),
        }];

        let in_flight_ids: Vec<String> = vec![];

        let prompt = analyzer.build_parallelization_prompt(&queued_changes, &in_flight_ids);

        // Verify queued changes are in the main list (no approval markers)
        assert!(prompt.contains("- queued-a (openspec/changes/queued-a/proposal.md)"));

        // Verify no executing section when in_flight_ids is empty
        assert!(!prompt.contains("Currently executing changes"));
    }

    /// Deterministic request construction for the optional shadow judge.
    ///
    /// These exercise the pure request/pair-mapping decisions plus the bounded
    /// proposal-read failure paths. No judge process is involved.
    mod shadow_judge_request {
        use super::*;
        use crate::config::{JudgeCommandsConfig, ParallelDependencyJudgeConfig};

        pub(super) fn judge_entry() -> ParallelDependencyJudgeConfig {
            ParallelDependencyJudgeConfig {
                command: vec!["unused-in-request-tests".to_string()],
                model: "test-judge-1.0.0".to_string(),
                timeout_ms: None,
                max_input_bytes: None,
                max_output_bytes: None,
                yes_threshold: None,
            }
        }

        pub(super) fn judge_config(
            judge: ParallelDependencyJudgeConfig,
        ) -> crate::config::OrchestratorConfig {
            crate::config::OrchestratorConfig {
                analyze_command: Some("echo unused".to_string()),
                judge_commands: Some(JudgeCommandsConfig {
                    parallel_dependency: Some(judge),
                }),
                ..Default::default()
            }
        }

        /// Create `openspec/changes/<id>/proposal.md` under `repo` with `body`.
        pub(super) fn write_proposal(repo: &Path, change_id: &str, body: &str) {
            let dir = repo.join("openspec").join("changes").join(change_id);
            std::fs::create_dir_all(&dir).expect("create change dir");
            std::fs::write(dir.join("proposal.md"), body).expect("write proposal");
        }

        fn change_with_metadata(
            id: &str,
            dependencies: Vec<&str>,
            priority: Option<crate::openspec::ProposalPriority>,
            references: Vec<&str>,
        ) -> Change {
            Change {
                id: id.to_string(),
                completed_tasks: 0,
                total_tasks: 5,
                last_modified: "now".to_string(),
                dependencies: dependencies.iter().map(|d| d.to_string()).collect(),
                metadata: ProposalMetadata {
                    priority,
                    references: references.iter().map(|r| r.to_string()).collect(),
                    ..Default::default()
                },
            }
        }

        #[test]
        fn request_asks_every_permitted_directed_pair_with_indexed_ids() {
            let repo = tempfile::TempDir::new().expect("tempdir");
            write_proposal(repo.path(), "queued-a", "proposal a");
            write_proposal(repo.path(), "queued-b", "proposal b");
            write_proposal(repo.path(), "inflight-c", "proposal c");
            let analyzer =
                create_test_analyzer_with(judge_config(judge_entry()), repo.path().to_path_buf());
            let changes = vec![
                change_with_metadata(
                    "queued-a",
                    vec!["queued-b"],
                    Some(crate::openspec::ProposalPriority::High),
                    vec!["src/analyzer.rs"],
                ),
                change_with_metadata("queued-b", vec![], None, vec![]),
            ];

            let (request, pairs) = analyzer
                .build_dependency_judge_request(
                    &judge_entry(),
                    &changes,
                    &["inflight-c".to_string()],
                )
                .expect("request construction must succeed")
                .expect("a permitted pair exists");

            // Dependent is always queued; the candidate is any other queued or
            // explicitly supplied in-flight node.
            let ids: Vec<&String> = request.questions.keys().collect();
            assert_eq!(
                ids,
                vec!["q_0000_0001", "q_0000_0002", "q_0001_0000", "q_0001_0002"],
                "in-flight nodes are dependency candidates only, never dependents"
            );
            assert_eq!(
                pairs.get("q_0000_0002"),
                Some(&("queued-a".to_string(), "inflight-c".to_string()))
            );
            assert_eq!(
                pairs.get("q_0001_0000"),
                Some(&("queued-b".to_string(), "queued-a".to_string()))
            );

            // Deterministic node ordering: queued in input order, then in-flight.
            assert_eq!(request.state.schema_version, 1);
            assert_eq!(
                request
                    .state
                    .queued_changes
                    .iter()
                    .map(|node| (node.index, node.id.as_str()))
                    .collect::<Vec<_>>(),
                vec![(0, "queued-a"), (1, "queued-b")]
            );
            assert_eq!(
                request
                    .state
                    .in_flight_changes
                    .iter()
                    .map(|node| (node.index, node.id.as_str()))
                    .collect::<Vec<_>>(),
                vec![(2, "inflight-c")]
            );

            // Structured state: complete proposal text plus separately carried,
            // still-authoritative metadata.
            assert_eq!(request.state.queued_changes[0].proposal, "proposal a");
            assert_eq!(
                request.state.queued_changes[0].metadata_dependencies,
                vec!["queued-b".to_string()]
            );
            assert_eq!(
                request.state.queued_changes[0].priority.as_deref(),
                Some("high")
            );
            assert_eq!(
                request.state.queued_changes[0].references,
                vec!["src/analyzer.rs".to_string()]
            );
            assert_eq!(request.state.in_flight_changes[0].proposal, "proposal c");

            // Pinned configured model, Noul type, and the official optional
            // true/false criteria.
            assert_eq!(request.model, "test-judge-1.0.0");
            let question = &request.questions["q_0000_0002"];
            assert_eq!(question.question_type, "noul");
            assert!(question.instructions.contains("`state.queued_changes[0]`"));
            assert!(question
                .instructions
                .contains("in-flight change `state.in_flight_changes[0]`"));
            assert!(question.instructions.contains("require repository output"));
            assert!(question.criteria.when_true.contains("contract"));
            assert!(
                question.criteria.when_false.contains("priority")
                    && question.criteria.when_false.contains("reference"),
                "priority and references must be excluded as dependency evidence"
            );

            // Byte-identical serialization for the same working set.
            let once = serde_json::to_string(&request).expect("serialize");
            let twice = serde_json::to_string(&request).expect("serialize");
            assert_eq!(once, twice);
        }

        #[test]
        fn untrusted_change_ids_cannot_reach_question_keys() {
            let repo = tempfile::TempDir::new().expect("tempdir");
            // Adversarial but path-safe IDs: one imitates a question key, the
            // other a protocol token.
            write_proposal(repo.path(), "q_0000_0001", "proposal one");
            write_proposal(repo.path(), "noul", "proposal two");
            let analyzer =
                create_test_analyzer_with(judge_config(judge_entry()), repo.path().to_path_buf());
            let changes = vec![
                create_test_change("q_0000_0001"),
                create_test_change("noul"),
            ];

            let (request, pairs) = analyzer
                .build_dependency_judge_request(&judge_entry(), &changes, &[])
                .expect("request construction must succeed")
                .expect("a permitted pair exists");

            assert_eq!(
                request.questions.keys().collect::<Vec<_>>(),
                vec!["q_0000_0001", "q_0001_0000"],
                "question keys are derived from trusted indexes only"
            );
            assert_eq!(
                pairs.get("q_0000_0001"),
                Some(&("q_0000_0001".to_string(), "noul".to_string())),
                "the pair map, not the key text, decides which pair a question is about"
            );
            let serialized = serde_json::to_value(&request).expect("serialize");
            assert_eq!(
                serialized["state"]["queued_changes"][0]["id"],
                "q_0000_0001"
            );
        }

        #[test]
        fn a_traversing_change_id_skips_the_complete_request() {
            let repo = tempfile::TempDir::new().expect("tempdir");
            write_proposal(repo.path(), "queued-a", "proposal a");
            let analyzer =
                create_test_analyzer_with(judge_config(judge_entry()), repo.path().to_path_buf());
            let changes = vec![
                create_test_change("queued-a"),
                create_test_change("../outside"),
            ];

            assert_eq!(
                analyzer
                    .build_dependency_judge_request(&judge_entry(), &changes, &[])
                    .err(),
                Some(JudgeFailureCategory::Io),
                "an ID that could leave the change layout is never read"
            );
        }

        #[test]
        fn unreadable_or_non_regular_proposals_skip_the_complete_request() {
            let repo = tempfile::TempDir::new().expect("tempdir");
            write_proposal(repo.path(), "queued-a", "proposal a");
            // `queued-b` has a change directory but no proposal file.
            std::fs::create_dir_all(repo.path().join("openspec/changes/queued-b"))
                .expect("create change dir");
            // `queued-c`'s proposal path is a directory, not a regular file.
            std::fs::create_dir_all(repo.path().join("openspec/changes/queued-c/proposal.md"))
                .expect("create non-regular proposal");
            let analyzer =
                create_test_analyzer_with(judge_config(judge_entry()), repo.path().to_path_buf());

            for missing in ["queued-b", "queued-c"] {
                let changes = vec![create_test_change("queued-a"), create_test_change(missing)];
                assert_eq!(
                    analyzer
                        .build_dependency_judge_request(&judge_entry(), &changes, &[])
                        .err(),
                    Some(JudgeFailureCategory::Io),
                    "{missing} must skip the request instead of being judged partially"
                );
            }
        }

        #[test]
        fn an_oversized_proposal_skips_the_complete_request() {
            let repo = tempfile::TempDir::new().expect("tempdir");
            write_proposal(repo.path(), "queued-a", "proposal a");
            write_proposal(repo.path(), "queued-b", &"x".repeat(64));
            let judge = ParallelDependencyJudgeConfig {
                max_input_bytes: Some(16),
                ..judge_entry()
            };
            let analyzer =
                create_test_analyzer_with(judge_config(judge.clone()), repo.path().to_path_buf());
            let changes = vec![
                create_test_change("queued-a"),
                create_test_change("queued-b"),
            ];

            assert_eq!(
                analyzer
                    .build_dependency_judge_request(&judge, &changes, &[])
                    .err(),
                Some(JudgeFailureCategory::InputTooLarge)
            );
        }

        #[test]
        fn a_single_node_has_no_permitted_pair() {
            let repo = tempfile::TempDir::new().expect("tempdir");
            write_proposal(repo.path(), "queued-a", "proposal a");
            let analyzer =
                create_test_analyzer_with(judge_config(judge_entry()), repo.path().to_path_buf());

            let built = analyzer
                .build_dependency_judge_request(
                    &judge_entry(),
                    &[create_test_change("queued-a")],
                    &[],
                )
                .expect("a working set with nothing to compare is not a failure");

            assert!(built.is_none(), "one node cannot form a directed pair");
        }

        #[test]
        fn judge_is_not_started_without_configuration_or_with_llm_analysis_disabled() {
            let repo = tempfile::TempDir::new().expect("tempdir");
            write_proposal(repo.path(), "queued-a", "proposal a");
            write_proposal(repo.path(), "queued-b", "proposal b");
            let changes = vec![
                create_test_change("queued-a"),
                create_test_change("queued-b"),
            ];

            let without_config = create_test_analyzer_with(
                crate::config::OrchestratorConfig::default(),
                repo.path().to_path_buf(),
            );
            assert!(
                without_config
                    .start_dependency_judge(&changes, &[])
                    .is_none(),
                "an absent judge configuration starts nothing"
            );

            let llm_disabled = create_test_analyzer_with(
                crate::config::OrchestratorConfig {
                    use_llm_analysis: Some(false),
                    ..judge_config(judge_entry())
                },
                repo.path().to_path_buf(),
            );
            assert!(
                llm_disabled.start_dependency_judge(&changes, &[]).is_none(),
                "a configured metadata-only result has nothing to shadow"
            );

            let invalid_entry = create_test_analyzer_with(
                judge_config(ParallelDependencyJudgeConfig {
                    command: Vec::new(),
                    ..judge_entry()
                }),
                repo.path().to_path_buf(),
            );
            assert!(
                invalid_entry
                    .start_dependency_judge(&changes, &[])
                    .is_none(),
                "an entry whose bounds were never validated is never spawned"
            );
        }

        #[tokio::test]
        async fn an_oversized_serialized_request_skips_the_spawn() {
            let repo = tempfile::TempDir::new().expect("tempdir");
            write_proposal(repo.path(), "queued-a", "proposal a");
            write_proposal(repo.path(), "queued-b", "proposal b");
            let judge = ParallelDependencyJudgeConfig {
                // Large enough for each proposal file, far too small for the
                // serialized request.
                max_input_bytes: Some(64),
                ..judge_entry()
            };
            let analyzer =
                create_test_analyzer_with(judge_config(judge), repo.path().to_path_buf());
            let changes = vec![
                create_test_change("queued-a"),
                create_test_change("queued-b"),
            ];

            assert!(
                analyzer.start_dependency_judge(&changes, &[]).is_none(),
                "the input limit is checked before any child is spawned"
            );
        }
    }

    /// Runtime behavior of the shadow judge beside the conventional analyzer.
    ///
    /// These drive a real child process through fake judge executables on
    /// purpose: concurrency, cancellation, reaping, and exit-code handling are
    /// properties of the process boundary. The pure decision logic they surround
    /// is covered by [`shadow_judge_request`] and the
    /// `crate::judge_command` unit doubles.
    #[cfg(unix)]
    mod shadow_judge_runtime {
        use super::shadow_judge_request::{judge_config, judge_entry, write_proposal};
        use super::*;
        use crate::config::{OrchestratorConfig, ParallelDependencyJudgeConfig};
        use std::os::unix::fs::PermissionsExt;

        const MODEL: &str = "test-judge-1.0.0";
        const ANALYZE_JSON: &str = r#"{"order":["queued-a","queued-b"],"dependencies":{}}"#;
        /// Marker planted in proposal text and judge stdout; diagnostics must
        /// never contain it.
        const SECRET: &str = "PROPOSAL-CONTENT-MUST-NOT-BE-LOGGED";

        fn valid_response() -> String {
            format!(
                r#"{{"model":"{MODEL}","answers":{{"q_0000_0001":{{"type":"noul","noul":0.9}},"q_0001_0000":{{"type":"noul","noul":0.1}}}},"usage":{{"input_tokens":11,"output_tokens":2}}}}"#
            )
        }

        /// Write an executable fake judge into the temporary repository.
        ///
        /// Every script first copies its stdin to `<repo>/<name>.request`, which
        /// is what makes "the judge ran" and "the judge received exactly this
        /// request" observable rather than assumed.
        fn write_fake_judge(repo: &Path, name: &str, body: &str) -> PathBuf {
            let path = repo.join(name);
            let script = format!(
                "#!/bin/sh\ncat > '{}'\n{body}\n",
                repo.join(format!("{name}.request")).display()
            );
            std::fs::write(&path, script).expect("write fake judge");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("chmod fake judge");
            path
        }

        fn judge_ran(repo: &Path, name: &str) -> bool {
            repo.join(format!("{name}.request")).exists()
        }

        /// A fake judge that publishes its response, then a completion marker.
        ///
        /// The conventional analyze command waits for that marker, so "the judge
        /// completed first" is a causal barrier rather than a timing hope.
        fn barrier_judge(repo: &Path, response: &str) -> PathBuf {
            write_fake_judge(
                repo,
                "barrier-judge",
                &format!(
                    "printf '%s' '{response}'\n: > '{}'",
                    repo.join("judge-done").display()
                ),
            )
        }

        fn analyze_after_judge(repo: &Path) -> String {
            analyze_after_marker(&repo.join("judge-done"))
        }

        /// A conventional analyze command that waits for `marker`, then answers.
        ///
        /// Used to make judge/analyzer ordering causal instead of timing-based.
        fn analyze_after_marker(marker: &Path) -> String {
            format!(
                "while [ ! -f '{}' ]; do sleep 0.01; done; printf '%s' '{ANALYZE_JSON}'",
                marker.display()
            )
        }

        fn runtime_config(analyze_command: String, judge_argv: Vec<String>) -> OrchestratorConfig {
            OrchestratorConfig {
                analyze_command: Some(analyze_command),
                // Keep the analyzer's own command path fast and single-shot: the
                // judge contract is what these tests are about.
                command_queue_stagger_delay_ms: Some(0),
                command_queue_max_retries: Some(0),
                ..judge_config(ParallelDependencyJudgeConfig {
                    command: judge_argv,
                    model: MODEL.to_string(),
                    // Far longer than any test: a caller that waited for the
                    // judge would hang instead of returning.
                    timeout_ms: Some(300_000),
                    ..judge_entry()
                })
            }
        }

        fn queued_pair() -> Vec<Change> {
            vec![
                create_test_change("queued-a"),
                create_test_change("queued-b"),
            ]
        }

        fn prepare_repo() -> tempfile::TempDir {
            let repo = tempfile::TempDir::new().expect("tempdir");
            write_proposal(repo.path(), "queued-a", &format!("proposal a {SECRET}"));
            write_proposal(repo.path(), "queued-b", &format!("proposal b {SECRET}"));
            repo
        }

        /// Fail fast instead of hanging if a judge ever became a blocking step.
        async fn analyze(analyzer: &ParallelizationAnalyzer, changes: &[Change]) -> AnalysisResult {
            tokio::time::timeout(
                Duration::from_secs(60),
                analyzer.analyze_with_inflight(changes, &[]),
            )
            .await
            .expect("analysis must not wait for the judge's own timeout")
            .expect("conventional analysis must succeed")
        }

        #[tokio::test]
        async fn a_valid_judge_leaves_the_authoritative_result_unchanged() {
            let repo = prepare_repo();
            let judge = barrier_judge(repo.path(), &valid_response());
            let with_judge = create_test_analyzer_with(
                runtime_config(
                    analyze_after_judge(repo.path()),
                    vec![judge.display().to_string()],
                ),
                repo.path().to_path_buf(),
            );
            let without_judge = create_test_analyzer_with(
                OrchestratorConfig {
                    analyze_command: Some(format!("printf '%s' '{ANALYZE_JSON}'")),
                    command_queue_stagger_delay_ms: Some(0),
                    command_queue_max_retries: Some(0),
                    ..Default::default()
                },
                repo.path().to_path_buf(),
            );

            let observed = analyze(&with_judge, &queued_pair()).await;
            let baseline = analyze(&without_judge, &queued_pair()).await;

            assert!(
                judge_ran(repo.path(), "barrier-judge"),
                "the judge must really have run for this comparison to mean anything"
            );
            assert_eq!(observed.order, baseline.order);
            assert_eq!(observed.dependencies, baseline.dependencies);
            assert!(
                observed.dependencies.is_empty(),
                "a judge `yes` never adds an authoritative dependency"
            );
            // The judge received the complete request on stdin.
            let request = std::fs::read_to_string(repo.path().join("barrier-judge.request"))
                .expect("captured request");
            let request: serde_json::Value =
                serde_json::from_str(&request).expect("request is one JSON object");
            assert_eq!(request["model"], MODEL);
            assert!(request["questions"]["q_0000_0001"].is_object());
        }

        #[tokio::test]
        async fn metadata_dependencies_survive_a_negative_judgment() {
            let repo = prepare_repo();
            // The judge says "no" for both directions, including the declared
            // metadata dependency queued-a → queued-b.
            let response = format!(
                r#"{{"model":"{MODEL}","answers":{{"q_0000_0001":{{"type":"noul","noul":0.0}},"q_0001_0000":{{"type":"noul","noul":0.0}}}},"usage":{{"input_tokens":1,"output_tokens":1}}}}"#
            );
            let judge = barrier_judge(repo.path(), &response);
            let analyzer = create_test_analyzer_with(
                runtime_config(
                    analyze_after_judge(repo.path()),
                    vec![judge.display().to_string()],
                ),
                repo.path().to_path_buf(),
            );
            let mut changes = queued_pair();
            changes[0].dependencies = vec!["queued-b".to_string()];

            let result = analyze(&analyzer, &changes).await;

            assert_eq!(
                result.dependencies.get("queued-a"),
                Some(&vec!["queued-b".to_string()]),
                "proposal metadata stays authoritative regardless of the judge"
            );
        }

        #[tokio::test]
        async fn an_unhealthy_judge_returns_the_conventional_result() {
            let repo = prepare_repo();
            let cases = vec![
                (
                    "missing-executable",
                    vec![repo.path().join("not-installed").display().to_string()],
                ),
                (
                    "nonzero-exit",
                    vec![write_fake_judge(repo.path(), "exit-judge", "exit 1")
                        .display()
                        .to_string()],
                ),
                (
                    "auth-failure",
                    vec![write_fake_judge(
                        repo.path(),
                        "auth-judge",
                        "echo 'credentials missing' >&2\nexit 3",
                    )
                    .display()
                    .to_string()],
                ),
                (
                    "malformed-output",
                    vec![write_fake_judge(
                        repo.path(),
                        "prose-judge",
                        "printf '%s' 'I think they are independent.'",
                    )
                    .display()
                    .to_string()],
                ),
                (
                    "wrong-model",
                    vec![write_fake_judge(
                        repo.path(),
                        "model-judge",
                        r#"printf '%s' '{"model":"other-9.9.9","answers":{},"usage":{"input_tokens":1,"output_tokens":1}}'"#,
                    )
                    .display()
                    .to_string()],
                ),
                (
                    "oversized-output",
                    vec![write_fake_judge(
                        repo.path(),
                        "loud-judge",
                        "i=0\nwhile [ $i -lt 500 ]; do printf '0123456789'; i=$((i+1)); done",
                    )
                    .display()
                    .to_string()],
                ),
            ];

            for (label, argv) in cases {
                let analyzer = create_test_analyzer_with(
                    OrchestratorConfig {
                        judge_commands: Some(crate::config::JudgeCommandsConfig {
                            parallel_dependency: Some(ParallelDependencyJudgeConfig {
                                command: argv,
                                model: MODEL.to_string(),
                                max_output_bytes: Some(64),
                                timeout_ms: Some(300_000),
                                ..judge_entry()
                            }),
                        }),
                        ..runtime_config(
                            format!("printf '%s' '{ANALYZE_JSON}'"),
                            vec!["unused".to_string()],
                        )
                    },
                    repo.path().to_path_buf(),
                );

                let result = analyze(&analyzer, &queued_pair()).await;

                assert_eq!(
                    result.order,
                    vec!["queued-a".to_string(), "queued-b".to_string()],
                    "{label} must leave the conventional result intact"
                );
                assert!(result.dependencies.is_empty(), "{label}");
            }
        }

        #[tokio::test]
        async fn a_slow_judge_never_delays_the_conventional_result() {
            let repo = prepare_repo();
            // Ignores TERM, so cleanup has to escalate to KILL after the fixed
            // grace — and the caller must not be waiting for any of it.
            let judge = write_fake_judge(repo.path(), "slow-judge", "trap '' TERM\nsleep 300");
            // The conventional command waits for the judge to be really running,
            // so the cancellation path is exercised rather than a spawn race.
            let analyzer = create_test_analyzer_with(
                runtime_config(
                    analyze_after_marker(&repo.path().join("slow-judge.request")),
                    vec![judge.display().to_string()],
                ),
                repo.path().to_path_buf(),
            );

            let result = analyze(&analyzer, &queued_pair()).await;

            assert!(
                judge_ran(repo.path(), "slow-judge"),
                "the slow judge must really have started"
            );
            assert_eq!(
                result.order,
                vec!["queued-a".to_string(), "queued-b".to_string()],
                "returning at all is the assertion: waiting for this judge would \
                 have blocked on its 5-minute timeout"
            );
        }

        #[tokio::test]
        async fn a_conventional_error_is_returned_unchanged_while_a_judge_runs() {
            let repo = prepare_repo();
            let judge = write_fake_judge(repo.path(), "running-judge", "sleep 300");
            let analyzer = create_test_analyzer_with(
                runtime_config(
                    format!(
                        "while [ ! -f '{}' ]; do sleep 0.01; done; printf '%s' 'not json at all'",
                        repo.path().join("running-judge.request").display()
                    ),
                    vec![judge.display().to_string()],
                ),
                repo.path().to_path_buf(),
            );

            let error = tokio::time::timeout(
                Duration::from_secs(60),
                analyzer.analyze_with_inflight(&queued_pair(), &[]),
            )
            .await
            .expect("an analyzer error must not wait for the judge")
            .expect_err("invalid conventional output must still fail");

            assert!(
                judge_ran(repo.path(), "running-judge"),
                "the judge must have been running when the analyzer failed"
            );
            assert!(
                error.to_string().contains("Analysis returned invalid JSON"),
                "the existing analyzer error must be returned unchanged: {error}"
            );
        }

        #[tokio::test]
        async fn early_return_paths_never_spawn_the_judge() {
            let repo = prepare_repo();
            let judge = write_fake_judge(repo.path(), "never-judge", "printf '%s' '{}'");
            let analyzer = create_test_analyzer_with(
                runtime_config(
                    format!("printf '%s' '{ANALYZE_JSON}'"),
                    vec![judge.display().to_string()],
                ),
                repo.path().to_path_buf(),
            );

            let empty = analyzer
                .analyze_with_inflight(&[], &[])
                .await
                .expect("zero changes");
            assert!(empty.order.is_empty());

            let single = analyzer
                .analyze_with_inflight(&[create_test_change("queued-a")], &[])
                .await
                .expect("single change");
            assert_eq!(single.order, vec!["queued-a".to_string()]);

            assert!(
                !judge_ran(repo.path(), "never-judge"),
                "the zero-change and single-change fast paths must start no judge"
            );
        }

        #[tokio::test]
        async fn diagnostics_are_aggregate_and_redacted() {
            let repo = prepare_repo();
            // The judge echoes the secret marker on stdout as well, so a leak
            // through raw process output would also be caught.
            let judge = barrier_judge(repo.path(), &valid_response());
            let analyzer = create_test_analyzer_with(
                runtime_config(
                    analyze_after_judge(repo.path()),
                    vec![judge.display().to_string()],
                ),
                repo.path().to_path_buf(),
            );

            let (capture, _guard) = capture_analyzer_tracing().await;
            let result = analyze(&analyzer, &queued_pair()).await;
            let records = capture.records();

            assert!(result.dependencies.is_empty());
            let observation = records
                .iter()
                .find(|(_, fields)| fields.contains("outcome=\"observed\""))
                .map(|(_, fields)| fields.clone())
                .expect("a completed judge must emit exactly one observation record");

            for allowed in [
                "purpose=\"parallel_dependency\"",
                "duration_ms=",
                "expected_answers=2",
                "returned_answers=2",
                "matching_positive_edges=0",
                "judge_only_positive_edges=1",
                "analyzer_only_positive_edges=0",
                "matching_negative_pairs=1",
                "total_pairs=2",
            ] {
                assert!(
                    observation.contains(allowed),
                    "the observation must carry {allowed}: {observation}"
                );
            }
            assert!(
                observation.contains(MODEL),
                "the validated model identity is allowed: {observation}"
            );

            for (_, fields) in &records {
                assert!(
                    !fields.contains(SECRET),
                    "no diagnostic may carry proposal text or raw judge output: {fields}"
                );
                assert!(
                    !fields.contains("queued-a") || !fields.contains("noul"),
                    "no diagnostic may tie a judgment value to a named change: {fields}"
                );
            }
        }

        #[tokio::test]
        async fn a_bounded_judge_failure_emits_one_redacted_category() {
            let repo = prepare_repo();
            let judge = write_fake_judge(
                repo.path(),
                "prose-judge",
                &format!("printf '%s' 'they are independent because {SECRET}'"),
            );
            let analyzer = create_test_analyzer_with(
                runtime_config(
                    format!(
                        "while [ ! -f '{}' ]; do sleep 0.01; done; printf '%s' '{ANALYZE_JSON}'",
                        repo.path().join("prose-judge.request").display()
                    ),
                    vec![judge.display().to_string()],
                ),
                repo.path().to_path_buf(),
            );

            let (capture, _guard) = capture_analyzer_tracing().await;
            let result = analyze(&analyzer, &queued_pair()).await;
            let records = capture.records();

            assert_eq!(
                result.order,
                vec!["queued-a".to_string(), "queued-b".to_string()]
            );
            assert!(
                records
                    .iter()
                    .any(|(level, fields)| *level == tracing::Level::WARN
                        && fields.contains("outcome=\"invalid_json\"")
                        && fields.contains("expected_answers=2")),
                "a malformed response is reported as one bounded category: {records:?}"
            );
            for (_, fields) in &records {
                assert!(
                    !fields.contains(SECRET),
                    "a failure diagnostic may not carry raw stdout: {fields}"
                );
                assert!(
                    !fields.contains("model="),
                    "an unvalidated model identity is not published: {fields}"
                );
            }
        }

        /// Collects tracing records so the judge's observability surface can be
        /// asserted on, including what it must never contain.
        #[derive(Clone, Default)]
        pub(super) struct CaptureLayer(
            std::sync::Arc<std::sync::Mutex<Vec<(tracing::Level, String)>>>,
        );

        impl CaptureLayer {
            fn records(&self) -> Vec<(tracing::Level, String)> {
                self.0.lock().expect("capture layer mutex").clone()
            }
        }

        impl<S> tracing_subscriber::Layer<S> for CaptureLayer
        where
            S: tracing::Subscriber,
        {
            fn on_event(
                &self,
                event: &tracing::Event<'_>,
                _ctx: tracing_subscriber::layer::Context<'_, S>,
            ) {
                struct Visitor {
                    fields: String,
                }

                impl tracing::field::Visit for Visitor {
                    fn record_debug(
                        &mut self,
                        field: &tracing::field::Field,
                        value: &dyn std::fmt::Debug,
                    ) {
                        self.fields
                            .push_str(&format!("{}={:?};", field.name(), value));
                    }
                }

                let mut visitor = Visitor {
                    fields: String::new(),
                };
                event.record(&mut visitor);
                self.0
                    .lock()
                    .expect("capture layer mutex")
                    .push((*event.metadata().level(), visitor.fields));
            }
        }

        /// Everything that must stay alive while a capture is collected.
        pub(super) struct TracingCapture {
            _exclusive: tokio::sync::MutexGuard<'static, ()>,
            _subscriber: tracing::subscriber::DefaultGuard,
        }

        /// Install an INFO-scoped capturing subscriber for the current test.
        async fn capture_analyzer_tracing() -> (CaptureLayer, TracingCapture) {
            use tracing_subscriber::filter::LevelFilter;
            use tracing_subscriber::layer::SubscriberExt;
            use tracing_subscriber::Layer;

            let exclusive = crate::test_support::tracing_capture_lock().lock().await;
            let capture = CaptureLayer::default();
            let subscriber =
                tracing_subscriber::registry().with(capture.clone().with_filter(LevelFilter::INFO));
            let subscriber = tracing::subscriber::set_default(subscriber);
            crate::test_support::refresh_tracing_interest();
            (
                capture,
                TracingCapture {
                    _exclusive: exclusive,
                    _subscriber: subscriber,
                },
            )
        }
    }
}
