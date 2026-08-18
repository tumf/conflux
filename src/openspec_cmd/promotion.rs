use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Parsed delta sections: ADDED, MODIFIED, REMOVED — each maps requirement name to block text.
#[derive(Debug, Default)]
pub(super) struct DeltaSections {
    pub(super) added: Vec<(String, String)>,
    pub(super) modified: Vec<(String, String)>,
    pub(super) removed: Vec<(String, String)>,
}

/// Split spec content into (preamble, [(name, full_block), ...]).
pub(super) fn split_spec(content: &str) -> (String, Vec<(String, String)>) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?m)^### Requirement:").unwrap());

    let mut blocks: Vec<(String, String)> = Vec::new();

    let parts: Vec<&str> = re.split(content).collect();
    // The regex splits at the start of "### Requirement:" so we need to re-attach
    let starts: Vec<_> = re.find_iter(content).collect();

    if parts.is_empty() {
        return (content.to_string(), blocks);
    }

    let preamble = parts[0].to_string();

    for (i, _start) in starts.iter().enumerate() {
        let block_body = if i < parts.len() - 1 {
            parts[i + 1]
        } else {
            ""
        };
        let full_block = format!("### Requirement:{}", block_body);

        // Extract the key from the heading line
        let heading_line = full_block.lines().next().unwrap_or("");
        let key = heading_line
            .strip_prefix("### Requirement:")
            .unwrap_or("")
            .trim()
            .to_string();

        if !key.is_empty() {
            blocks.push((key, full_block));
        }
    }

    (preamble, blocks)
}

/// Parse delta content into sections (ADDED, MODIFIED, REMOVED).
pub(super) fn parse_delta_sections(delta: &str) -> DeltaSections {
    static SECTION_RE: OnceLock<Regex> = OnceLock::new();
    let section_re = SECTION_RE
        .get_or_init(|| Regex::new(r"(?m)^## (ADDED|MODIFIED|REMOVED) Requirements\s*$").unwrap());

    let matches: Vec<_> = section_re.find_iter(delta).collect();
    let caps: Vec<_> = section_re.captures_iter(delta).collect();

    let mut sections = DeltaSections::default();

    for (i, cap) in caps.iter().enumerate() {
        let section_type = cap.get(1).unwrap().as_str();
        let start = matches[i].end();
        let end = if i + 1 < matches.len() {
            matches[i + 1].start()
        } else {
            delta.len()
        };
        let section_content = &delta[start..end];
        let (_, blocks) = split_spec(section_content);

        match section_type {
            "ADDED" => sections.added.extend(blocks),
            "MODIFIED" => sections.modified.extend(blocks),
            "REMOVED" => sections.removed.extend(blocks),
            _ => {}
        }
    }

    sections
}

/// Reconstruct a spec from preamble and requirement blocks.
pub(super) fn reconstruct(preamble: &str, blocks: &[(String, String)]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let trimmed_preamble = preamble.trim_end_matches('\n');
    if !trimmed_preamble.trim().is_empty() {
        parts.push(trimmed_preamble.to_string());
    }
    for (_, block) in blocks {
        parts.push(block.trim_end_matches('\n').to_string());
    }
    let mut result = parts.join("\n\n");
    if !result.is_empty() && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Check if two block lists are equal (same keys and stripped content).
pub(super) fn blocks_equal(b1: &[(String, String)], b2: &[(String, String)]) -> bool {
    if b1.len() != b2.len() {
        return false;
    }
    b1.iter()
        .zip(b2.iter())
        .all(|((k1, v1), (k2, v2))| k1 == k2 && v1.trim() == v2.trim())
}

/// Merge a change delta into a canonical spec.
///
/// Returns (result_content, errors). When errors is non-empty, the canonical
/// content is returned unchanged.
pub fn merge_spec_delta(canonical: &str, delta: &str) -> (String, Vec<String>) {
    let mut errors: Vec<String> = Vec::new();
    let sections = parse_delta_sections(delta);
    let (preamble, original_blocks) = split_spec(canonical);
    let original_dict: HashMap<&str, &str> = original_blocks
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    // Validate MODIFIED targets
    for (key, _) in &sections.modified {
        if !original_dict.contains_key(key.as_str()) {
            errors.push(format!(
                "MODIFIED target not found in canonical spec: '### Requirement: {}'",
                key
            ));
        }
    }

    // Validate REMOVED targets
    for (key, _) in &sections.removed {
        if !original_dict.contains_key(key.as_str()) {
            errors.push(format!(
                "REMOVED target not found in canonical spec: '### Requirement: {}'",
                key
            ));
        }
    }

    if !errors.is_empty() {
        return (canonical.to_string(), errors);
    }

    let removed_keys: std::collections::HashSet<&str> =
        sections.removed.iter().map(|(k, _)| k.as_str()).collect();
    let modified_map: HashMap<&str, &str> = sections
        .modified
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();

    // Apply REMOVED and MODIFIED to original blocks (preserve order)
    let mut result_blocks: Vec<(String, String)> = Vec::new();
    for (key, block) in &original_blocks {
        if removed_keys.contains(key.as_str()) {
            continue; // delete
        } else if let Some(new_block) = modified_map.get(key.as_str()) {
            result_blocks.push((key.clone(), new_block.to_string())); // replace
        } else {
            result_blocks.push((key.clone(), block.clone())); // keep
        }
    }

    // Append ADDED blocks at the end
    for (key, block) in &sections.added {
        result_blocks.push((key.clone(), block.clone()));
    }

    // Reject no-op promotions
    if blocks_equal(&original_blocks, &result_blocks) {
        errors
            .push("Archive promotion would produce no canonical diff (no-op archive)".to_string());
        return (canonical.to_string(), errors);
    }

    (reconstruct(&preamble, &result_blocks), Vec::new())
}

/// Convert a delta-format spec to canonical format for brand-new specs.
pub fn delta_to_canonical(delta: &str) -> Result<String, String> {
    let sections = parse_delta_sections(delta);
    let mut all_blocks: Vec<(String, String)> = Vec::new();
    all_blocks.extend(sections.added);
    all_blocks.extend(sections.modified);
    all_blocks.extend(sections.removed);

    if all_blocks.is_empty() {
        return Err(
            "Spec delta parse error: no canonical requirement blocks found for promotion"
                .to_string(),
        );
    }

    Ok(reconstruct("", &all_blocks))
}

/// Simulate spec promotion without writing files.
pub fn simulate_promotion(canonical: Option<&str>, delta: &str) -> (String, Vec<String>) {
    match canonical {
        None => match delta_to_canonical(delta) {
            Ok(canonicalized) => (canonicalized, Vec::new()),
            Err(err) => (delta.to_string(), vec![err]),
        },
        Some(canonical) => merge_spec_delta(canonical, delta),
    }
}

// ============================================================================
// Promotion safety
// ============================================================================
//
// Promotion replaces a canonical requirement block wholesale with the delta's
// version. That is the right primitive — a delta is the new text of the
// requirement — but it makes one specific accident silent and expensive: a
// MODIFIED block that forgot to carry an existing scenario deletes that
// scenario from the canonical spec, and nothing about the merge looks wrong.
// The archive is where it is noticed, which is after the change is gone.
//
// So the property is stated here, as a check anyone can run before promotion:
// every scenario a canonical requirement has today must still be there after
// the delta is applied, unless the delta explicitly retired that requirement
// through a REMOVED section.

/// One requirement's scenario titles, in document order.
///
/// Titles rather than bodies: a scenario may legitimately be reworded by a
/// change, but a scenario that *disappears* is a coverage loss, and the title is
/// the identity a reviewer tracks it by.
// Read by the promotion-safety regression below. Deliberately not wired into
// `validate --archive-gate`: this change adds the check, not a new gate, and
// turning it into one would start refusing archives for a rule no proposal has
// been reviewed against yet.
#[cfg_attr(not(test), allow(dead_code))]
pub fn scenarios_by_requirement(spec: &str) -> Vec<(String, Vec<String>)> {
    let (_preamble, blocks) = split_spec(spec);
    blocks
        .into_iter()
        .map(|(name, block)| {
            let scenarios = block
                .lines()
                .filter_map(|line| line.strip_prefix("#### Scenario:"))
                .map(|title| title.trim().to_string())
                .collect();
            (name, scenarios)
        })
        .collect()
}

/// One scenario that promotion would drop without a REMOVED declaration.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedScenario {
    /// The canonical requirement that owns it.
    pub requirement: String,
    /// The scenario title that no longer appears.
    pub scenario: String,
}

/// Scenarios the canonical spec has today and the promoted spec would not.
///
/// A requirement the delta retires through `## REMOVED Requirements` is exempt:
/// removing it is the declared intent, and its scenarios go with it. Everything
/// else is a coverage regression, whether the delta meant it or not.
///
/// Retention is judged against the *whole promoted spec*, not against the
/// requirement the scenario started in. Moving a scenario to a better-fitting
/// requirement is ordinary editing and loses no coverage, and a rule that
/// reported it would train reviewers to ignore the check. The requirement it
/// came from is still reported, because that is what a reader needs to find it.
#[cfg_attr(not(test), allow(dead_code))]
pub fn dropped_scenarios(canonical: &str, delta: &str) -> Vec<DroppedScenario> {
    let retired: std::collections::BTreeSet<String> = parse_delta_sections(delta)
        .removed
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    let (promoted, _errors) = merge_spec_delta(canonical, delta);
    let surviving: std::collections::BTreeSet<String> = scenarios_by_requirement(&promoted)
        .into_iter()
        .flat_map(|(_, scenarios)| scenarios)
        .collect();

    let mut dropped = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (requirement, scenarios) in scenarios_by_requirement(canonical) {
        if retired.contains(&requirement) {
            continue;
        }
        for scenario in scenarios {
            if surviving.contains(&scenario) {
                continue;
            }
            // A canonical spec may carry the same requirement heading more than
            // once. Reporting per *block* would name the same loss twice.
            if seen.insert((requirement.clone(), scenario.clone())) {
                dropped.push(DroppedScenario {
                    requirement: requirement.clone(),
                    scenario,
                });
            }
        }
    }
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical() -> String {
        [
            "## Requirements",
            "",
            "### Requirement: Kept whole",
            "",
            "Text.",
            "",
            "#### Scenario: First",
            "",
            "- **WHEN** something",
            "- **THEN** something else",
            "",
            "#### Scenario: Second",
            "",
            "- **WHEN** something",
            "- **THEN** something else",
            "",
            "### Requirement: Retired",
            "",
            "Text.",
            "",
            "#### Scenario: Only",
            "",
            "- **WHEN** something",
            "- **THEN** something else",
            "",
        ]
        .join("\n")
    }

    #[test]
    fn scenarios_are_read_per_requirement_in_document_order() {
        let grouped = scenarios_by_requirement(&canonical());
        assert_eq!(
            grouped,
            vec![
                (
                    "Kept whole".to_string(),
                    vec!["First".to_string(), "Second".to_string()]
                ),
                ("Retired".to_string(), vec!["Only".to_string()]),
            ]
        );
    }

    /// The accident the check exists for: a MODIFIED block that carries only the
    /// scenario its author was thinking about silently deletes the other one.
    #[test]
    fn a_modified_requirement_that_forgets_a_scenario_is_reported_as_a_drop() {
        let delta = [
            "## MODIFIED Requirements",
            "",
            "### Requirement: Kept whole",
            "",
            "New text.",
            "",
            "#### Scenario: First",
            "",
            "- **WHEN** something",
            "- **THEN** something else",
            "",
        ]
        .join("\n");
        assert_eq!(
            dropped_scenarios(&canonical(), &delta),
            vec![DroppedScenario {
                requirement: "Kept whole".to_string(),
                scenario: "Second".to_string(),
            }]
        );
    }

    #[test]
    fn a_modified_requirement_that_carries_every_scenario_drops_nothing() {
        let delta = [
            "## MODIFIED Requirements",
            "",
            "### Requirement: Kept whole",
            "",
            "New text.",
            "",
            "#### Scenario: First",
            "",
            "- **WHEN** something",
            "- **THEN** something else",
            "",
            "#### Scenario: Second",
            "",
            "- **WHEN** something",
            "- **THEN** something else",
            "",
            "#### Scenario: Added by this change",
            "",
            "- **WHEN** something",
            "- **THEN** something else",
            "",
        ]
        .join("\n");
        assert!(dropped_scenarios(&canonical(), &delta).is_empty());
    }

    /// A retired requirement takes its scenarios with it, and that is declared
    /// rather than inferred: only a `## REMOVED Requirements` entry exempts one.
    #[test]
    fn an_explicitly_removed_requirement_is_exempt_and_only_that_one_is() {
        let delta = [
            "## REMOVED Requirements",
            "",
            "### Requirement: Retired",
            "",
            "**Reason**: superseded.",
            "",
            "**Migration**: use the other one.",
            "",
        ]
        .join("\n");
        assert!(
            dropped_scenarios(&canonical(), &delta).is_empty(),
            "the removal was declared"
        );

        // The same removal without the declaration is a drop.
        let undeclared = [
            "## MODIFIED Requirements",
            "",
            "### Requirement: Retired",
            "",
            "Text.",
            "",
        ]
        .join("\n");
        assert_eq!(
            dropped_scenarios(&canonical(), &undeclared),
            vec![DroppedScenario {
                requirement: "Retired".to_string(),
                scenario: "Only".to_string(),
            }]
        );
    }

    /// An ADDED requirement introduces scenarios and removes none.
    #[test]
    fn a_proposal_declares_its_intentional_retirements_by_scenario_title() {
        let proposal = [
            "# A change",
            "",
            "## Retired Scenarios",
            "",
            "- remote-control-api: Client observation / Enqueue uses ordinary typed commands",
            "- cli: Something else / Another retired title",
            "",
            "## Out of Scope",
            "",
            "- cli: Not retired / This heading ended the section",
            "",
        ]
        .join("\n");
        let declared = declared_retirements(&proposal);
        assert!(declared.contains("Enqueue uses ordinary typed commands"));
        assert!(declared.contains("Another retired title"));
        assert!(
            !declared.contains("This heading ended the section"),
            "a later section is not part of the declaration"
        );
        assert!(declared_retirements("# A change with no declaration").is_empty());
    }

    /// A canonical spec may repeat a requirement heading. A change that touches
    /// neither block must not be accused of deleting a scenario one of them
    /// still holds.
    #[test]
    fn duplicate_requirement_headings_are_compared_as_one_name() {
        let repeated = [
            "## Requirements",
            "",
            "### Requirement: Repeated",
            "",
            "#### Scenario: From the first block",
            "",
            "- **WHEN** something",
            "",
            "### Requirement: Repeated",
            "",
            "#### Scenario: From the second block",
            "",
            "- **WHEN** something",
            "",
        ]
        .join("\n");
        let unrelated = [
            "## ADDED Requirements",
            "",
            "### Requirement: Brand new",
            "",
            "#### Scenario: Fresh",
            "",
            "- **WHEN** something",
            "",
        ]
        .join("\n");
        assert!(
            dropped_scenarios(&repeated, &unrelated).is_empty(),
            "neither block was touched, so neither lost anything"
        );
    }

    /// Moving a scenario to a better-fitting requirement loses no coverage, so
    /// it is not a drop — the check is about the spec, not about where in it a
    /// scenario lives.
    #[test]
    fn a_scenario_moved_to_another_requirement_is_still_retained() {
        let delta = [
            "## MODIFIED Requirements",
            "",
            "### Requirement: Kept whole",
            "",
            "New text.",
            "",
            "#### Scenario: First",
            "",
            "- **WHEN** something",
            "",
            "## ADDED Requirements",
            "",
            "### Requirement: A better home",
            "",
            "#### Scenario: Second",
            "",
            "- **WHEN** something",
            "",
        ]
        .join("\n");
        assert!(
            dropped_scenarios(&canonical(), &delta).is_empty(),
            "'Second' moved rather than disappeared"
        );
    }

    #[test]
    fn an_added_requirement_never_reports_a_drop() {
        let delta = [
            "## ADDED Requirements",
            "",
            "### Requirement: Brand new",
            "",
            "Text.",
            "",
            "#### Scenario: Fresh",
            "",
            "- **WHEN** something",
            "- **THEN** something else",
            "",
        ]
        .join("\n");
        assert!(dropped_scenarios(&canonical(), &delta).is_empty());
    }

    /// Scenario titles a change declares it is retiring on purpose.
    ///
    /// Declared in `proposal.md` rather than in the delta, deliberately: a delta
    /// block is copied verbatim into the canonical spec, so a declaration living
    /// there would become permanent noise in a document that should describe the
    /// system rather than the history of one change. The proposal is
    /// change-scoped and reviewed with the delta, which is exactly where an
    /// intent statement belongs.
    fn declared_retirements(proposal: &str) -> std::collections::BTreeSet<String> {
        let mut retired = std::collections::BTreeSet::new();
        let mut inside = false;
        for line in proposal.lines() {
            if line.starts_with("## ") {
                inside = line.trim() == "## Retired Scenarios";
                continue;
            }
            if !inside {
                continue;
            }
            if let Some(entry) = line.trim().strip_prefix("- ") {
                // `capability: Requirement / Scenario title`
                if let Some((_, title)) = entry.rsplit_once('/') {
                    retired.insert(title.trim().to_string());
                }
            }
        }
        retired
    }

    /// The regression itself, run against the repository's own pending changes.
    ///
    /// The delta files *are* the subject, so an in-memory fixture here would
    /// test a copy rather than the thing that will be promoted. The read is of
    /// tracked source at a fixed path — deterministic, and nothing else mutates
    /// it while the test runs.
    ///
    /// It stays correct after this change is archived: an archived change is not
    /// under `openspec/changes/`, so the scan simply finds one fewer — and a
    /// repository with nothing pending is the ordinary end state, not a broken
    /// harness. What the scan must not do is go quiet because its own roots
    /// moved, so the roots are asserted and the count is not.
    #[test]
    fn every_pending_change_promotes_without_dropping_a_scenario() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let changes = root.join("openspec/changes");
        let canonical_root = root.join("openspec/specs");
        assert!(
            changes.is_dir() && canonical_root.is_dir(),
            "the scan cannot find its own roots ({} and {}), so it would silently check nothing",
            changes.display(),
            canonical_root.display()
        );
        let entries = std::fs::read_dir(&changes).expect("openspec/changes is readable");

        for entry in entries.flatten() {
            let change = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name == "archive" || !change.is_dir() {
                continue;
            }
            let specs = change.join("specs");
            let Ok(capabilities) = std::fs::read_dir(&specs) else {
                continue;
            };
            for capability in capabilities.flatten() {
                let capability_name = capability.file_name().to_string_lossy().to_string();
                let delta_path = capability.path().join("spec.md");
                let Ok(delta) = std::fs::read_to_string(&delta_path) else {
                    continue;
                };
                let canonical_path = canonical_root.join(&capability_name).join("spec.md");
                // A brand-new capability has no canonical spec to regress.
                let Ok(canonical) = std::fs::read_to_string(&canonical_path) else {
                    continue;
                };
                let declared = std::fs::read_to_string(change.join("proposal.md"))
                    .map(|proposal| declared_retirements(&proposal))
                    .unwrap_or_default();
                let dropped: Vec<DroppedScenario> = dropped_scenarios(&canonical, &delta)
                    .into_iter()
                    .filter(|drop| !declared.contains(&drop.scenario))
                    .collect();
                assert!(
                    dropped.is_empty(),
                    "{name}/{capability_name} would delete canonical scenarios that it declared \
                     neither as a REMOVED requirement nor under `## Retired Scenarios` in its \
                     proposal: {dropped:#?}"
                );
            }
        }
    }
}
