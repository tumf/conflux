use crate::openspec_cmd::promotion::{delta_to_canonical, merge_spec_delta, simulate_promotion};
use chrono::Local;
use std::fs;
use std::path::Path;

pub(super) struct ArchiveEngine<'a> {
    pub(super) manager: &'a super::OpenSpecManager,
}

impl<'a> ArchiveEngine<'a> {
    pub(super) fn archive_change(
        &self,
        change_id: &str,
        skip_specs: bool,
    ) -> Result<String, String> {
        let change_dir = self.manager.changes_dir.join(change_id);

        if !change_dir.exists() {
            return Err(format!("Change '{}' not found", change_id));
        }

        if change_dir.to_string_lossy().contains("/archive/") {
            return Err(format!("Change '{}' is already archived", change_id));
        }

        // Validate before archiving
        let (is_valid, errors, _) = self.manager.validate_change(Some(change_id), true, "error");
        if !is_valid {
            return Err(format!("Validation failed:\n{}", errors.join("\n")));
        }

        // Simulate spec promotion
        if !skip_specs {
            let sim_errors = self.simulate_spec_promotion(&change_dir);
            if !sim_errors.is_empty() {
                return Err(format!(
                    "Spec promotion simulation failed:\n{}",
                    sim_errors.join("\n")
                ));
            }
        }

        // Create archive directory
        fs::create_dir_all(&self.manager.archive_dir)
            .map_err(|e| format!("Failed to create archive directory: {}", e))?;

        // Move to archive using dated destination (YYYY-MM-DD-<change_id>)
        let archive_name = format!("{}-{}", Local::now().format("%Y-%m-%d"), change_id);
        let archive_dest = self.manager.archive_dir.join(&archive_name);
        if archive_dest.exists() {
            return Err(format!(
                "Archive destination already exists: {}",
                archive_dest.display()
            ));
        }

        fs::rename(&change_dir, &archive_dest)
            .map_err(|e| format!("Failed to move to archive: {}", e))?;

        // Update specs
        if !skip_specs {
            let specs_updated = self.update_specs_from_change(&archive_dest);
            Ok(format!(
                "Archived to openspec/changes/archive/{}\nSpecs updated: {:?}",
                archive_name, specs_updated
            ))
        } else {
            Ok(format!(
                "Archived to openspec/changes/archive/{}",
                archive_name
            ))
        }
    }

    fn simulate_spec_promotion(&self, change_dir: &Path) -> Vec<String> {
        let mut errors = Vec::new();
        let specs_dir = change_dir.join("specs");
        if !specs_dir.exists() {
            return errors;
        }

        if let Ok(entries) = fs::read_dir(&specs_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let spec_file = path.join("spec.md");
                if !spec_file.exists() {
                    continue;
                }

                let spec_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let canonical_spec = self.manager.specs_dir.join(&spec_name).join("spec.md");
                let canonical_content = if canonical_spec.exists() {
                    Some(fs::read_to_string(&canonical_spec).unwrap_or_default())
                } else {
                    None
                };
                let delta_content = match fs::read_to_string(&spec_file) {
                    Ok(c) => c,
                    Err(e) => {
                        errors.push(format!("{}: Failed to read delta: {}", spec_name, e));
                        continue;
                    }
                };

                let (_, sim_errors) =
                    simulate_promotion(canonical_content.as_deref(), &delta_content);
                for err in sim_errors {
                    errors.push(format!("{}: {}", spec_name, err));
                }
            }
        }

        errors
    }

    fn update_specs_from_change(&self, change_dir: &Path) -> Vec<String> {
        let mut updated = Vec::new();
        let specs_dir = change_dir.join("specs");

        if !specs_dir.exists() {
            return updated;
        }

        if let Ok(entries) = fs::read_dir(&specs_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let spec_file = path.join("spec.md");
                if !spec_file.exists() {
                    continue;
                }

                let spec_name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                let canonical_dir = self.manager.specs_dir.join(&spec_name);
                let canonical_spec = canonical_dir.join("spec.md");

                // Create parent dir
                let _ = fs::create_dir_all(&canonical_dir);

                let delta_content = match fs::read_to_string(&spec_file) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if canonical_spec.exists() {
                    let canonical_content = fs::read_to_string(&canonical_spec).unwrap_or_default();
                    let (merged, errors) = merge_spec_delta(&canonical_content, &delta_content);
                    if errors.is_empty() {
                        let _ = fs::write(&canonical_spec, merged);
                        updated.push(spec_name);
                    } else {
                        eprintln!(
                            "parse error/promotion error for spec '{}': {}",
                            spec_name,
                            errors.join("; ")
                        );
                    }
                } else {
                    match delta_to_canonical(&delta_content) {
                        Ok(canonicalized) => {
                            let _ = fs::write(&canonical_spec, canonicalized);
                            updated.push(spec_name);
                        }
                        Err(err) => {
                            eprintln!(
                                "parse error/promotion error for spec '{}': {}",
                                spec_name, err
                            );
                        }
                    }
                }
            }
        }

        updated
    }
}
