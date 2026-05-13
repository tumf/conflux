use crate::openspec_cmd::model::{ChangeInfo, DependencyStatusInfo, ShowInfo, SpecInfo};

pub(super) fn render_specs_output(specs: &[SpecInfo]) -> String {
    let mut output = String::from("\n\x1b[1mSpecifications:\x1b[0m\n\n");
    for spec in specs {
        output.push_str(&format!("  \x1b[96m{}\x1b[0m\n", spec.name));
        output.push_str(&format!("    Path: {}\n", spec.path));
        output.push_str(&format!("    Requirements: {}\n\n", spec.requirement_count));
    }
    output
}

pub(super) fn format_dependency_statuses(dependency_statuses: &[DependencyStatusInfo]) -> String {
    dependency_statuses
        .iter()
        .map(|dependency| format!("{} [{}]", dependency.id, dependency.status.label()))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn render_changes_output(changes: &[ChangeInfo]) -> String {
    let mut output = String::from("\n\x1b[1mChanges:\x1b[0m\n\n");
    for change in changes {
        output.push_str(&format!(
            "  \x1b[92m[ACTIVE]\x1b[0m \x1b[1m{}\x1b[0m\n",
            change.id
        ));
        if let Some(ref title) = change.title {
            output.push_str(&format!("    Title: {}\n", title));
        }
        if change.tasks_total > 0 {
            let progress = format!("{}/{}", change.tasks_completed, change.tasks_total);
            if change.tasks_completed == change.tasks_total {
                output.push_str(&format!("    Tasks: \x1b[92m{}\x1b[0m\n", progress));
            } else {
                output.push_str(&format!("    Tasks: {}\n", progress));
            }
        }
        if !change.dependency_statuses.is_empty() {
            output.push_str(&format!(
                "    Dependencies: {}\n",
                format_dependency_statuses(&change.dependency_statuses)
            ));
        }
        output.push_str(&format!("    Path: {}\n\n", change.path));
    }
    output
}

pub(super) fn truncate_for_display(content: &str, max_chars: usize) -> String {
    let truncated: String = content.chars().take(max_chars).collect();
    if content.chars().count() > max_chars {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

pub(super) fn render_show_json_value(info: &ShowInfo) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    map.insert("id".to_string(), serde_json::Value::String(info.id.clone()));
    map.insert(
        "path".to_string(),
        serde_json::Value::String(info.path.clone()),
    );
    map.insert(
        "archived".to_string(),
        serde_json::Value::Bool(info.archived),
    );

    if let Some(ref proposal) = info.proposal {
        map.insert(
            "proposal".to_string(),
            serde_json::Value::String(proposal.clone()),
        );
    }
    if let Some(ref tasks) = info.tasks {
        map.insert(
            "tasks".to_string(),
            serde_json::Value::String(tasks.clone()),
        );
        map.insert(
            "tasks_completed".to_string(),
            serde_json::Value::Number(info.tasks_completed.into()),
        );
        map.insert(
            "tasks_total".to_string(),
            serde_json::Value::Number(info.tasks_total.into()),
        );
    }
    if let Some(ref design) = info.design {
        map.insert(
            "design".to_string(),
            serde_json::Value::String(design.clone()),
        );
    }
    if !info.dependencies.is_empty() {
        let dependencies = info
            .dependency_statuses
            .iter()
            .map(|dependency| {
                let mut dep_map = serde_json::Map::new();
                dep_map.insert(
                    "id".to_string(),
                    serde_json::Value::String(dependency.id.clone()),
                );
                dep_map.insert(
                    "status".to_string(),
                    serde_json::Value::String(dependency.status.label().to_string()),
                );
                serde_json::Value::Object(dep_map)
            })
            .collect::<Vec<_>>();
        map.insert(
            "dependencies".to_string(),
            serde_json::Value::Array(dependencies),
        );
    }
    if !info.specs.is_empty() {
        let specs_map: serde_json::Map<String, serde_json::Value> = info
            .specs
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();
        map.insert("specs".to_string(), serde_json::Value::Object(specs_map));
    }

    serde_json::Value::Object(map)
}

pub(super) fn render_show_output(info: &ShowInfo) -> String {
    let mut output = String::new();
    output.push_str(&format!("\n\x1b[1mChange: {}\x1b[0m\n", info.id));
    output.push_str(&format!("Path: {}\n", info.path));
    output.push_str(&format!(
        "Status: {}\n",
        if info.archived { "ARCHIVED" } else { "ACTIVE" }
    ));

    if info.tasks.is_some() {
        output.push_str(&format!(
            "Tasks: {}/{}\n",
            info.tasks_completed, info.tasks_total
        ));
    }

    if !info.dependency_statuses.is_empty() {
        output.push_str(&format!(
            "Dependencies: {}\n",
            format_dependency_statuses(&info.dependency_statuses)
        ));
    }

    if let Some(ref proposal) = info.proposal {
        output.push_str("\n\x1b[1mProposal:\x1b[0m\n");
        output.push_str(&format!("{}\n", truncate_for_display(proposal, 500)));
    }

    if !info.specs.is_empty() {
        output.push_str("\n\x1b[1mSpec Deltas:\x1b[0m\n");
        for (name, content) in &info.specs {
            output.push_str(&format!("\n  \x1b[96m{}:\x1b[0m\n", name));
            output.push_str(&format!("  {}\n", truncate_for_display(content, 300)));
        }
    }

    output
}
