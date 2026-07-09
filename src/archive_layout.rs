use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArchiveLayoutError {
    pub(crate) change_id: String,
    pub(crate) path: PathBuf,
}

impl ArchiveLayoutError {
    pub(crate) fn message(&self) -> String {
        format!(
            "Invalid archive layout for '{}': found nested archive path {}. Expected openspec/changes/archive/YYYY-MM-DD-{}. Do not manually move archive directories; restore the active change and rerun cflx openspec archive {} --yes.",
            self.change_id,
            self.path.display(),
            self.change_id,
            self.change_id
        )
    }
}

pub(crate) fn find_valid_archive_entry(change_id: &str, archive_dir: &Path) -> Option<PathBuf> {
    if !archive_dir.exists() {
        return None;
    }

    let direct = archive_dir.join(change_id);
    if direct.is_dir() {
        return Some(direct);
    }

    fs::read_dir(archive_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            is_valid_dated_archive_name(&name, change_id).then_some(path)
        })
}

pub(crate) fn find_invalid_nested_archive_entry(
    change_id: &str,
    archive_dir: &Path,
) -> Option<PathBuf> {
    if !archive_dir.exists() {
        return None;
    }

    fs::read_dir(archive_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .find_map(|entry| {
            let date_dir = entry.path();
            if !date_dir.is_dir() {
                return None;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !is_yyyy_mm_dd(&name) {
                return None;
            }
            let nested = date_dir.join(change_id);
            nested.is_dir().then_some(nested)
        })
}

pub(crate) fn invalid_layout_error(
    change_id: &str,
    archive_dir: &Path,
) -> Option<ArchiveLayoutError> {
    find_invalid_nested_archive_entry(change_id, archive_dir).map(|path| ArchiveLayoutError {
        change_id: change_id.to_string(),
        path,
    })
}

pub(crate) fn is_valid_archive_entry_name(name: &str, change_id: &str) -> bool {
    name == change_id || is_valid_dated_archive_name(name, change_id)
}

fn is_valid_dated_archive_name(name: &str, change_id: &str) -> bool {
    let Some(date) = name.strip_suffix(&format!("-{change_id}")) else {
        return false;
    };
    is_yyyy_mm_dd(date)
}

fn is_yyyy_mm_dd(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_archive_names_are_direct_or_dated() {
        assert!(is_valid_archive_entry_name("my-change", "my-change"));
        assert!(is_valid_archive_entry_name(
            "2026-07-09-my-change",
            "my-change"
        ));
        assert!(!is_valid_archive_entry_name(
            "prefix-my-change",
            "my-change"
        ));
        assert!(!is_valid_archive_entry_name(
            "2026-7-9-my-change",
            "my-change"
        ));
    }

    #[test]
    fn nested_archive_entry_is_invalid_layout() {
        let dir = tempfile::tempdir().unwrap();
        let archive_dir = dir.path().join("openspec/changes/archive");
        let nested = archive_dir.join("2026-07-09/my-change");
        std::fs::create_dir_all(&nested).unwrap();

        let err = invalid_layout_error("my-change", &archive_dir).unwrap();
        assert_eq!(err.path, nested);
        assert!(err.message().contains("Invalid archive layout"));
        assert!(err.message().contains("2026-07-09/my-change"));
    }
}
