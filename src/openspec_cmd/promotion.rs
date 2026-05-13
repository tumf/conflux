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
