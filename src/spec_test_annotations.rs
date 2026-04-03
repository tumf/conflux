use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Error)]
pub enum SpecTestAnnotationError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Spec parsing error: {0}")]
    Parse(String),
}

type Result<T> = std::result::Result<T, SpecTestAnnotationError>;

static REQUIREMENT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^### Requirement:\s*(.+)\s*$").unwrap());
static SCENARIO_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#### Scenario:\s*(.+)\s*$").unwrap());
static UI_ONLY_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)ui[-_ ]?only").unwrap());
static ANNOTATION_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"//\s*OPENSPEC:\s*(\S+)").unwrap());

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SpecScenarioRef {
    spec_path: String,
    requirement_slug: String,
    scenario_slug: String,
}

impl SpecScenarioRef {
    fn new(spec_path: String, requirement_slug: String, scenario_slug: String) -> Self {
        Self {
            spec_path,
            requirement_slug,
            scenario_slug,
        }
    }

    fn format_reference(&self) -> String {
        format!(
            "{}#{}/{}",
            self.spec_path, self.requirement_slug, self.scenario_slug
        )
    }
}

impl fmt::Display for SpecScenarioRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_reference())
    }
}

#[derive(Debug, Clone)]
struct SpecScenario {
    reference: SpecScenarioRef,
    ui_only: bool,
}

#[derive(Debug, Clone)]
struct SpecReference {
    reference: SpecScenarioRef,
    location: String,
}

#[derive(Debug, Default)]
struct AnnotationScanResult {
    references: Vec<SpecReference>,
    broken: Vec<BrokenReference>,
}

#[derive(Debug, Clone)]
struct BrokenReference {
    reference: String,
    location: String,
    reason: String,
}

#[derive(Debug, Default)]
struct SpecCheckReport {
    missing: Vec<SpecScenarioRef>,
    broken: Vec<BrokenReference>,
}

impl SpecCheckReport {
    fn format_report(&self) -> String {
        let mut output = Vec::new();
        if !self.missing.is_empty() {
            output.push("Missing spec coverage:".to_string());
            for missing in &self.missing {
                output.push(format!("  - {}", missing));
            }
        }
        if !self.broken.is_empty() {
            if !output.is_empty() {
                output.push("".to_string());
            }
            output.push("Broken spec references:".to_string());
            for broken in &self.broken {
                output.push(format!(
                    "  - {} ({}): {}",
                    broken.reference, broken.location, broken.reason
                ));
            }
        }
        output.join("\n")
    }
}

fn slugify_heading(input: &str) -> String {
    let normalized = input.nfkc().collect::<String>().to_lowercase();
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in normalized.chars() {
        if ch.is_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.starts_with('-') {
        slug.remove(0);
    }
    while slug.ends_with('-') {
        slug.pop();
    }

    slug
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[allow(clippy::too_many_arguments)]
fn finalize_scenario(
    scenario_title: Option<String>,
    lines: &[String],
    current_requirement: &Option<String>,
    current_requirement_slug: &Option<String>,
    spec_path_string: &str,
    spec_path: &Path,
    scenarios: &mut Vec<SpecScenario>,
) -> Result<()> {
    if let Some(scenario_title) = scenario_title {
        let requirement_title = current_requirement.clone().ok_or_else(|| {
            SpecTestAnnotationError::Parse(format!(
                "Scenario '{}' appears before any requirement in {}",
                scenario_title,
                spec_path.display()
            ))
        })?;
        let requirement_slug = current_requirement_slug
            .clone()
            .unwrap_or_else(|| slugify_heading(&requirement_title));
        let scenario_slug = slugify_heading(&scenario_title);
        let ui_only = lines.iter().any(|line| UI_ONLY_REGEX.is_match(line));

        scenarios.push(SpecScenario {
            reference: SpecScenarioRef::new(
                spec_path_string.to_string(),
                requirement_slug,
                scenario_slug,
            ),
            ui_only,
        });
    }
    Ok(())
}

fn collect_spec_files(dir: &Path, specs: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_spec_files(&path, specs)?;
        } else if path.file_name().is_some_and(|name| name == "spec.md") {
            specs.push(path);
        }
    }
    Ok(())
}

fn parse_spec_file(spec_path: &Path, repo_root: &Path) -> Result<Vec<SpecScenario>> {
    let content = fs::read_to_string(spec_path)?;

    let spec_path_relative = spec_path
        .strip_prefix(repo_root)
        .map_err(|_| {
            SpecTestAnnotationError::Parse(format!(
                "Spec path '{}' is outside repo root",
                spec_path.display()
            ))
        })?
        .to_path_buf();
    let spec_path_string = normalize_path(&spec_path_relative);

    let mut scenarios = Vec::new();
    let mut current_requirement: Option<String> = None;
    let mut current_requirement_slug: Option<String> = None;
    let mut current_scenario: Option<String> = None;
    let mut current_lines: Vec<String> = Vec::new();

    for line in content.lines() {
        if let Some(caps) = REQUIREMENT_REGEX.captures(line) {
            finalize_scenario(
                current_scenario.take(),
                &current_lines,
                &current_requirement,
                &current_requirement_slug,
                &spec_path_string,
                spec_path,
                &mut scenarios,
            )?;
            current_lines.clear();
            let requirement_title = caps[1].trim().to_string();
            current_requirement_slug = Some(slugify_heading(&requirement_title));
            current_requirement = Some(requirement_title);
            continue;
        }

        if let Some(caps) = SCENARIO_REGEX.captures(line) {
            finalize_scenario(
                current_scenario.take(),
                &current_lines,
                &current_requirement,
                &current_requirement_slug,
                &spec_path_string,
                spec_path,
                &mut scenarios,
            )?;
            current_lines.clear();
            current_scenario = Some(caps[1].trim().to_string());
            continue;
        }

        if current_scenario.is_some() {
            current_lines.push(line.to_string());
        }
    }

    finalize_scenario(
        current_scenario.take(),
        &current_lines,
        &current_requirement,
        &current_requirement_slug,
        &spec_path_string,
        spec_path,
        &mut scenarios,
    )?;
    Ok(scenarios)
}

fn collect_spec_scenarios(repo_root: &Path) -> Result<Vec<SpecScenario>> {
    let spec_root = repo_root.join("openspec").join("specs");
    let mut spec_files = Vec::new();
    collect_spec_files(&spec_root, &mut spec_files)?;

    let mut scenarios = Vec::new();
    for spec_file in spec_files {
        scenarios.extend(parse_spec_file(&spec_file, repo_root)?);
    }
    Ok(scenarios)
}

fn collect_rs_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn parse_reference(reference: &str) -> Result<SpecScenarioRef> {
    let (spec_path, rest) = reference.split_once('#').ok_or_else(|| {
        SpecTestAnnotationError::Parse(format!("Missing '#' in reference: {}", reference))
    })?;
    let (requirement_slug, scenario_slug) = rest.split_once('/').ok_or_else(|| {
        SpecTestAnnotationError::Parse(format!("Missing '/' in reference: {}", reference))
    })?;
    if requirement_slug.is_empty() || scenario_slug.is_empty() {
        return Err(SpecTestAnnotationError::Parse(format!(
            "Empty slug in reference: {}",
            reference
        )));
    }

    Ok(SpecScenarioRef::new(
        spec_path.to_string(),
        requirement_slug.to_string(),
        scenario_slug.to_string(),
    ))
}

fn collect_annotations(repo_root: &Path) -> Result<AnnotationScanResult> {
    let mut rs_files = Vec::new();
    for code_root in ["src", "tests"] {
        let path = repo_root.join(code_root);
        if path.exists() {
            collect_rs_files(&path, &mut rs_files)?;
        }
    }

    let mut result = AnnotationScanResult::default();

    for file_path in rs_files {
        let content = fs::read_to_string(&file_path)?;
        for (index, line) in content.lines().enumerate() {
            if let Some(caps) = ANNOTATION_REGEX.captures(line) {
                let reference_text = caps.get(1).unwrap().as_str();
                let location = format!("{}:{}", normalize_path(&file_path), index + 1);
                match parse_reference(reference_text) {
                    Ok(reference) => result.references.push(SpecReference {
                        reference,
                        location,
                    }),
                    Err(err) => result.broken.push(BrokenReference {
                        reference: reference_text.to_string(),
                        location,
                        reason: err.to_string(),
                    }),
                }
            }
        }
    }

    Ok(result)
}

fn check_spec_test_annotations(repo_root: &Path) -> Result<SpecCheckReport> {
    let scenarios = collect_spec_scenarios(repo_root)?;
    let annotation_result = collect_annotations(repo_root)?;
    let references = annotation_result.references;

    let mut scenario_map: HashMap<SpecScenarioRef, bool> = HashMap::new();
    for scenario in &scenarios {
        scenario_map.insert(scenario.reference.clone(), scenario.ui_only);
    }

    let mut referenced = HashSet::new();
    for reference in &references {
        referenced.insert(reference.reference.clone());
    }

    let mut report = SpecCheckReport {
        missing: Vec::new(),
        broken: annotation_result.broken,
    };

    for scenario in scenarios {
        if scenario.ui_only {
            continue;
        }
        if !referenced.contains(&scenario.reference) {
            report.missing.push(scenario.reference);
        }
    }

    for reference in references {
        if !scenario_map.contains_key(&reference.reference) {
            report.broken.push(BrokenReference {
                reference: reference.reference.format_reference(),
                location: reference.location,
                reason: "Reference does not match any spec scenario".to_string(),
            });
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify_heading() {
        assert_eq!(slugify_heading("run Subcommand"), "run-subcommand");
        assert_eq!(
            slugify_heading("Running 中に queued-change を外す"),
            "running-中に-queued-change-を外す"
        );
    }

    #[test]
    fn test_spec_annotation_checker_reports_missing_and_broken() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let spec_dir = root.join("openspec/specs/testing");
        fs::create_dir_all(&spec_dir).unwrap();
        fs::write(
            spec_dir.join("spec.md"),
            r#"### Requirement: Sample Requirement

#### Scenario: Covered scenario
- **WHEN** something happens

#### Scenario: Missing scenario
- **WHEN** something else happens

#### Scenario: UI-only scenario
UI-only
"#,
        )
        .unwrap();

        let tests_dir = root.join("tests");
        fs::create_dir_all(&tests_dir).unwrap();
        fs::write(
            tests_dir.join("sample.rs"),
            r#"// OPENSPEC: openspec/specs/testing/spec.md#sample-requirement/covered-scenario
// OPENSPEC: openspec/specs/testing/spec.md#sample-requirement/unknown-scenario
#[test]
fn example() {}
"#,
        )
        .unwrap();

        let report = check_spec_test_annotations(root).unwrap();
        let missing_refs: Vec<String> = report.missing.iter().map(|r| r.to_string()).collect();
        let broken_refs: Vec<String> = report.broken.iter().map(|r| r.reference.clone()).collect();

        assert!(missing_refs.contains(
            &"openspec/specs/testing/spec.md#sample-requirement/missing-scenario".to_string()
        ));
        assert!(!missing_refs.contains(
            &"openspec/specs/testing/spec.md#sample-requirement/ui-only-scenario".to_string()
        ));
        assert!(broken_refs.contains(
            &"openspec/specs/testing/spec.md#sample-requirement/unknown-scenario".to_string()
        ));
    }

    #[test]
    fn test_format_report_includes_sections() {
        let report = SpecCheckReport {
            missing: vec![SpecScenarioRef::new(
                "openspec/specs/testing/spec.md".to_string(),
                "req".to_string(),
                "scenario".to_string(),
            )],
            broken: vec![BrokenReference {
                reference: "openspec/specs/testing/spec.md#req/bad".to_string(),
                location: "tests/sample.rs:1".to_string(),
                reason: "Reference does not match any spec scenario".to_string(),
            }],
        };

        let output = report.format_report();
        assert!(output.contains("Missing spec coverage:"));
        assert!(output.contains("Broken spec references:"));
        assert!(output.contains("openspec/specs/testing/spec.md#req/scenario"));
        assert!(output.contains("tests/sample.rs:1"));
    }
}
