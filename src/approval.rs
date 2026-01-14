//! Approval workflow module
//!
//! This module provides functionality for managing change approval status.
//! An approved change has a manifest file containing MD5 checksums of all
//! specification files, which is used to validate that the change hasn't
//! been modified since approval.

use crate::error::{OrchestratorError, Result};
use crate::tui::log_deduplicator;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use tracing::debug;

/// Discover all markdown files in a change directory.
///
/// Scans `openspec/changes/{change_id}/` recursively for `*.md` files,
/// excluding `tasks.md` which changes during execution.
///
/// # Arguments
/// * `change_id` - The ID of the change to scan
///
/// # Returns
/// A sorted vector of file paths relative to the project root
pub fn discover_md_files(change_id: &str) -> Result<Vec<PathBuf>> {
    let change_dir = PathBuf::from("openspec/changes").join(change_id);

    if !change_dir.exists() {
        return Err(OrchestratorError::ConfigLoad(format!(
            "Change directory does not exist: {}",
            change_dir.display()
        )));
    }

    let mut files = Vec::new();
    discover_md_files_recursive(&change_dir, &mut files)?;

    // Filter out tasks.md
    files.retain(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n != "tasks.md")
            .unwrap_or(true)
    });

    // Sort for consistent ordering
    files.sort();

    debug!(
        "Discovered {} markdown files for change '{}'",
        files.len(),
        change_id
    );
    Ok(files)
}

/// Recursively discover markdown files in a directory
fn discover_md_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(dir).map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to read directory {}: {}", dir.display(), e))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            OrchestratorError::ConfigLoad(format!("Failed to read directory entry: {}", e))
        })?;

        let path = entry.path();

        if path.is_dir() {
            discover_md_files_recursive(&path, files)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            files.push(path);
        }
    }

    Ok(())
}

/// Compute MD5 checksum of a file
///
/// # Arguments
/// * `path` - Path to the file
///
/// # Returns
/// Hexadecimal MD5 hash string
pub fn compute_md5(path: &Path) -> Result<String> {
    let content = fs::read(path).map_err(|e| {
        OrchestratorError::ConfigLoad(format!("Failed to read file {}: {}", path.display(), e))
    })?;

    let hash = md5::compute(&content);
    Ok(format!("{:x}", hash))
}

/// Parse an approved file to get the file-to-hash mapping
///
/// # Arguments
/// * `path` - Path to the approved file
///
/// # Returns
/// A map from file path to expected MD5 hash
#[allow(dead_code)]
pub fn parse_approved_file(path: &Path) -> Result<BTreeMap<PathBuf, String>> {
    let file = fs::File::open(path).map_err(|e| {
        OrchestratorError::ConfigLoad(format!(
            "Failed to open approved file {}: {}",
            path.display(),
            e
        ))
    })?;

    let reader = BufReader::new(file);
    let mut manifest = BTreeMap::new();

    for line in reader.lines() {
        let line = line.map_err(|e| {
            OrchestratorError::ConfigLoad(format!("Failed to read line from approved file: {}", e))
        })?;

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse md5sum format: "hash  path" (two spaces between hash and path)
        if let Some((hash, path_str)) = line.split_once("  ") {
            let file_path = PathBuf::from(path_str);
            manifest.insert(file_path, hash.to_string());
        }
    }

    Ok(manifest)
}

/// Check if a change is approved
///
/// A change is approved if:
/// 1. The `approved` file exists
///
/// # Arguments
/// * `change_id` - The ID of the change to check
///
/// # Returns
/// True if the change is approved and valid, false otherwise
pub fn check_approval(change_id: &str) -> Result<bool> {
    let approved_path = PathBuf::from("openspec/changes")
        .join(change_id)
        .join("approved");

    // 1. Check if approved file exists
    if !approved_path.exists() {
        if log_deduplicator::should_log_approval_status(change_id, false) {
            debug!("Change '{}' is not approved: no approved file", change_id);
        }
        return Ok(false);
    }

    // 2. Parse approved manifest
    let manifest = match parse_approved_file(&approved_path) {
        Ok(m) => m,
        Err(e) => {
            debug!(
                "Change '{}' approval invalid: failed to parse manifest: {}",
                change_id, e
            );
            return Ok(false);
        }
    };

    // 3. Get current file list (excluding tasks.md)
    let current_files = match discover_md_files(change_id) {
        Ok(f) => f,
        Err(e) => {
            debug!(
                "Change '{}' approval check failed: cannot discover files: {}",
                change_id, e
            );
            return Err(e);
        }
    };

    // 4. Compare file lists (excluding tasks.md from manifest)
    let manifest_files: std::collections::HashSet<PathBuf> = manifest
        .keys()
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n != "tasks.md")
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    let current_set: std::collections::HashSet<PathBuf> = current_files.into_iter().collect();

    if manifest_files != current_set {
        debug!(
            "Change '{}' approval invalid: file list mismatch. Manifest: {:?}, Current: {:?}",
            change_id, manifest_files, current_set
        );
        return Ok(false);
    }

    // 5. Verify hashes for all files in manifest (except tasks.md)
    for (path, expected_hash) in &manifest {
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n == "tasks.md")
            .unwrap_or(false)
        {
            continue;
        }

        let actual_hash = match compute_md5(path) {
            Ok(h) => h,
            Err(e) => {
                debug!(
                    "Change '{}' approval invalid: cannot compute hash for {}: {}",
                    change_id,
                    path.display(),
                    e
                );
                return Ok(false);
            }
        };

        if &actual_hash != expected_hash {
            debug!(
                "Change '{}' approval invalid: hash mismatch for {}. Expected: {}, Actual: {}",
                change_id,
                path.display(),
                expected_hash,
                actual_hash
            );
            return Ok(false);
        }
    }

    if log_deduplicator::should_log_approval_status(change_id, true) {
        debug!("Change '{}' is approved and valid", change_id);
    }
    Ok(true)
}

/// Approve a change by creating the approved manifest file
///
/// # Arguments
/// * `change_id` - The ID of the change to approve
///
/// # Returns
/// Ok(()) on success
pub fn approve_change(change_id: &str) -> Result<()> {
    let change_dir = PathBuf::from("openspec/changes").join(change_id);

    if !change_dir.exists() {
        return Err(OrchestratorError::ConfigLoad(format!(
            "Change directory does not exist: {}",
            change_dir.display()
        )));
    }

    let approved_path = change_dir.join("approved");
    let files = discover_md_files(change_id)?;

    // Create manifest content
    let mut content = String::new();
    for file in &files {
        let hash = compute_md5(file)?;
        content.push_str(&format!("{}  {}\n", hash, file.display()));
    }

    // Write approved file
    let mut file = fs::File::create(&approved_path).map_err(|e| {
        OrchestratorError::ConfigLoad(format!(
            "Failed to create approved file {}: {}",
            approved_path.display(),
            e
        ))
    })?;

    file.write_all(content.as_bytes()).map_err(|e| {
        OrchestratorError::ConfigLoad(format!(
            "Failed to write approved file {}: {}",
            approved_path.display(),
            e
        ))
    })?;

    debug!("Approved change '{}' with {} files", change_id, files.len());
    Ok(())
}

/// Unapprove a change by removing the approved manifest file
///
/// # Arguments
/// * `change_id` - The ID of the change to unapprove
///
/// # Returns
/// Ok(()) on success, or if the approved file doesn't exist
pub fn unapprove_change(change_id: &str) -> Result<()> {
    let approved_path = PathBuf::from("openspec/changes")
        .join(change_id)
        .join("approved");

    if approved_path.exists() {
        fs::remove_file(&approved_path).map_err(|e| {
            OrchestratorError::ConfigLoad(format!(
                "Failed to remove approved file {}: {}",
                approved_path.display(),
                e
            ))
        })?;
        debug!("Unapproved change '{}'", change_id);
    } else {
        debug!(
            "Change '{}' was not approved (no approved file to remove)",
            change_id
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use tempfile::TempDir;

    #[allow(dead_code)]
    fn setup_test_change(temp_dir: &TempDir, change_id: &str) -> PathBuf {
        let changes_dir = temp_dir.path().join("openspec/changes").join(change_id);
        fs::create_dir_all(&changes_dir).unwrap();

        // Create test files
        fs::write(changes_dir.join("proposal.md"), "# Proposal\nTest content").unwrap();
        fs::write(changes_dir.join("design.md"), "# Design\nDesign content").unwrap();
        fs::write(changes_dir.join("tasks.md"), "# Tasks\n- [ ] Task 1").unwrap();

        // Create nested specs directory
        let specs_dir = changes_dir.join("specs");
        fs::create_dir_all(&specs_dir).unwrap();
        fs::write(specs_dir.join("spec.md"), "# Spec\nSpec content").unwrap();

        changes_dir
    }

    static CURRENT_DIR_LOCK: Mutex<()> = Mutex::new(());

    fn lock_current_dir() -> std::sync::MutexGuard<'static, ()> {
        CURRENT_DIR_LOCK
            .lock()
            .unwrap_or_else(|err| err.into_inner())
    }

    #[test]
    fn test_compute_md5() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        fs::write(&file_path, "test content").unwrap();

        let hash = compute_md5(&file_path).unwrap();
        assert_eq!(hash.len(), 32); // MD5 hex string length
        assert_eq!(hash, "9473fdd0d880a43c21b7778d34872157"); // Known hash for "test content"
    }

    #[test]
    fn test_parse_approved_file() {
        let temp_dir = TempDir::new().unwrap();
        let approved_path = temp_dir.path().join("approved");

        let content =
            "abc123  openspec/changes/test/proposal.md\ndef456  openspec/changes/test/design.md\n";
        fs::write(&approved_path, content).unwrap();

        let manifest = parse_approved_file(&approved_path).unwrap();
        assert_eq!(manifest.len(), 2);
        assert_eq!(
            manifest.get(&PathBuf::from("openspec/changes/test/proposal.md")),
            Some(&"abc123".to_string())
        );
        assert_eq!(
            manifest.get(&PathBuf::from("openspec/changes/test/design.md")),
            Some(&"def456".to_string())
        );
    }

    #[test]
    fn test_parse_approved_file_with_empty_lines() {
        let temp_dir = TempDir::new().unwrap();
        let approved_path = temp_dir.path().join("approved");

        let content = "abc123  path/to/file.md\n\n  \ndef456  path/to/other.md\n";
        fs::write(&approved_path, content).unwrap();

        let manifest = parse_approved_file(&approved_path).unwrap();
        assert_eq!(manifest.len(), 2);
    }

    // === Tests for approval spec (approval workflow) ===

    #[test]
    fn test_approved_file_is_md5sum_compatible_format() {
        // The approved file format should be compatible with md5sum
        // Format: "{hash}  {path}" (two spaces between hash and path)
        let temp_dir = TempDir::new().unwrap();
        let approved_path = temp_dir.path().join("approved");

        // This is the expected format output by md5sum
        let content = "d41d8cd98f00b204e9800998ecf8427e  openspec/changes/test/proposal.md\n";
        fs::write(&approved_path, content).unwrap();

        let manifest = parse_approved_file(&approved_path).unwrap();
        assert_eq!(manifest.len(), 1);
        assert_eq!(
            manifest.get(&PathBuf::from("openspec/changes/test/proposal.md")),
            Some(&"d41d8cd98f00b204e9800998ecf8427e".to_string())
        );
    }

    #[test]
    fn test_compute_md5_produces_32_char_hex() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.md");
        fs::write(&file_path, "any content here").unwrap();

        let hash = compute_md5(&file_path).unwrap();

        // MD5 hash should be 32 hex characters
        assert_eq!(hash.len(), 32);
        // All characters should be hex digits
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_compute_md5_same_content_same_hash() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("file1.md");
        let file2 = temp_dir.path().join("file2.md");

        let content = "identical content";
        fs::write(&file1, content).unwrap();
        fs::write(&file2, content).unwrap();

        let hash1 = compute_md5(&file1).unwrap();
        let hash2 = compute_md5(&file2).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_compute_md5_different_content_different_hash() {
        let temp_dir = TempDir::new().unwrap();
        let file1 = temp_dir.path().join("file1.md");
        let file2 = temp_dir.path().join("file2.md");

        fs::write(&file1, "content A").unwrap();
        fs::write(&file2, "content B").unwrap();

        let hash1 = compute_md5(&file1).unwrap();
        let hash2 = compute_md5(&file2).unwrap();

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_md5_file_not_found() {
        let result = compute_md5(&PathBuf::from("/nonexistent/path/to/file.md"));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_approved_file_not_found() {
        let result = parse_approved_file(&PathBuf::from("/nonexistent/approved"));
        assert!(result.is_err());
    }

    #[test]
    fn test_approved_manifest_sorted_by_path() {
        // BTreeMap keeps keys sorted, verifying the manifest uses BTreeMap
        let temp_dir = TempDir::new().unwrap();
        let approved_path = temp_dir.path().join("approved");

        // Write paths in reverse order
        let content = "ccc333  openspec/changes/test/z.md\nbbb222  openspec/changes/test/m.md\naaa111  openspec/changes/test/a.md\n";
        fs::write(&approved_path, content).unwrap();

        let manifest = parse_approved_file(&approved_path).unwrap();

        // BTreeMap should iterate in sorted order
        let paths: Vec<&PathBuf> = manifest.keys().collect();
        assert_eq!(paths[0].to_str().unwrap(), "openspec/changes/test/a.md");
        assert_eq!(paths[1].to_str().unwrap(), "openspec/changes/test/m.md");
        assert_eq!(paths[2].to_str().unwrap(), "openspec/changes/test/z.md");
    }

    #[test]
    fn test_discover_md_files_nonexistent_change() {
        // discover_md_files should return error for nonexistent change
        let result = discover_md_files("nonexistent-change-xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_check_approval_missing_approved_file() {
        // change exists but no approved file -> returns Ok(false)
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes/test-change");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::write(changes_dir.join("proposal.md"), "# Proposal").unwrap();

        let _lock = lock_current_dir();

        // Change to the temp dir so relative paths work
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = check_approval("test-change");

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(!result.unwrap()); // Not approved
    }

    // === Tests for approval/unapproval workflow ===

    #[test]
    fn test_approve_creates_approved_file() {
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes/my-change");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::write(changes_dir.join("proposal.md"), "# Proposal\nContent").unwrap();

        let _lock = lock_current_dir();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = approve_change("my-change");

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(changes_dir.join("approved").exists());
    }

    #[test]
    fn test_unapprove_removes_approved_file() {
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes/my-change");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::write(changes_dir.join("proposal.md"), "# Proposal\nContent").unwrap();
        fs::write(
            changes_dir.join("approved"),
            "abc123  openspec/changes/my-change/proposal.md\n",
        )
        .unwrap();

        let _lock = lock_current_dir();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = unapprove_change("my-change");

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        assert!(!changes_dir.join("approved").exists());
    }

    #[test]
    fn test_unapprove_already_unapproved_is_ok() {
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes/my-change");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::write(changes_dir.join("proposal.md"), "# Proposal\nContent").unwrap();
        // No approved file exists

        let _lock = lock_current_dir();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = unapprove_change("my-change");

        std::env::set_current_dir(original_dir).unwrap();

        // Should succeed even though no approved file existed
        assert!(result.is_ok());
    }

    #[test]
    fn test_approve_nonexistent_change_fails() {
        let temp_dir = TempDir::new().unwrap();
        let _lock = lock_current_dir();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let result = approve_change("nonexistent-change");

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_err());
    }

    #[test]
    fn test_approved_file_content_format() {
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes/test");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::write(changes_dir.join("proposal.md"), "content").unwrap();
        fs::write(changes_dir.join("design.md"), "design").unwrap();

        let _lock = lock_current_dir();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        approve_change("test").unwrap();

        let approved_content = fs::read_to_string(changes_dir.join("approved")).unwrap();

        std::env::set_current_dir(original_dir).unwrap();

        // Each line should be "{hash}  {path}" format
        for line in approved_content.lines() {
            if !line.is_empty() {
                let parts: Vec<&str> = line.split("  ").collect();
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0].len(), 32); // MD5 hash length
            }
        }
    }

    // === Tests for tasks.md exclusion ===

    #[test]
    fn test_discover_md_files_excludes_tasks_md() {
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes/my-change");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::write(changes_dir.join("proposal.md"), "# Proposal").unwrap();
        fs::write(changes_dir.join("tasks.md"), "# Tasks").unwrap();

        let _lock = lock_current_dir();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let files = discover_md_files("my-change").unwrap();

        std::env::set_current_dir(original_dir).unwrap();

        // tasks.md should be excluded
        assert!(!files
            .iter()
            .any(|f| f.file_name().map(|n| n == "tasks.md").unwrap_or(false)));
        assert!(files
            .iter()
            .any(|f| f.file_name().map(|n| n == "proposal.md").unwrap_or(false)));
    }

    #[test]
    fn test_check_approval_only_checks_approved_file() {
        let temp_dir = TempDir::new().unwrap();
        let changes_dir = temp_dir.path().join("openspec/changes/my-change");
        fs::create_dir_all(&changes_dir).unwrap();
        fs::write(changes_dir.join("proposal.md"), "# Proposal").unwrap();

        let _lock = lock_current_dir();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Create a valid approved manifest so check_approval can validate it.
        approve_change("my-change").unwrap();

        let is_approved = check_approval("my-change").unwrap();

        std::env::set_current_dir(original_dir).unwrap();

        assert!(is_approved);
    }
}
