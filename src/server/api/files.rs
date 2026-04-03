use super::*;

// ─────────────────────────── /api/v1/projects/:id/files ───────────────────────

/// Directories excluded from the file tree listing.
const EXCLUDED_DIRS: &[&str] = &[".git", "node_modules", ".next", "target", "dist"];

/// Maximum file content size returned by the content API (1 MB).
const MAX_FILE_CONTENT_SIZE: u64 = 1_048_576;

/// Query parameters for the file tree API.
#[derive(Debug, Deserialize)]
pub struct FileTreeQuery {
    /// `base` (default) or `worktree:<branch>`.
    #[serde(default = "default_root")]
    pub root: String,
}

fn default_root() -> String {
    "base".to_string()
}

/// Query parameters for the file content API.
#[derive(Debug, Deserialize)]
pub struct FileContentQuery {
    /// `base` (default) or `worktree:<branch>`.
    #[serde(default = "default_root")]
    pub root: String,
    /// Relative path to the file from the root.
    pub path: String,
}

/// A single entry in the file tree.
#[derive(Debug, Serialize)]
pub struct FileTreeEntry {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String, // "file" or "directory"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<FileTreeEntry>>,
}

/// Response for the file content API.
#[derive(Debug, Serialize)]
pub struct FileContentResponse {
    pub path: String,
    pub content: Option<String>,
    pub size: u64,
    pub truncated: bool,
    pub binary: bool,
}

/// Resolve the file-system root path for a given project and `root` query parameter.
///
/// Returns `Ok(path)` or an error `Response` ready to send back.
pub(super) async fn resolve_file_root(
    state: &AppState,
    project_id: &str,
    root_param: &str,
) -> Result<std::path::PathBuf, Response> {
    let registry = state.registry.read().await;
    let entry = registry.get(project_id).cloned().ok_or_else(|| {
        error_response(
            StatusCode::NOT_FOUND,
            format!("Project not found: {}", project_id),
        )
    })?;
    let data_dir = registry.data_dir().to_path_buf();

    if root_param == "base" || root_param.is_empty() {
        let base_path = data_dir
            .join("worktrees")
            .join(project_id)
            .join(&entry.branch);
        if !base_path.exists() {
            return Err(error_response(
                StatusCode::NOT_FOUND,
                "Base worktree not found",
            ));
        }
        Ok(base_path)
    } else if let Some(branch) = root_param.strip_prefix("worktree:") {
        // Look up the worktree path by branch name from git worktree list.
        let base_path = data_dir
            .join("worktrees")
            .join(project_id)
            .join(&entry.branch);
        if !base_path.exists() {
            return Err(error_response(
                StatusCode::NOT_FOUND,
                "Base worktree not found",
            ));
        }

        let worktrees = crate::worktree_ops::get_worktrees(&base_path)
            .await
            .map_err(|e| {
                error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Failed to list worktrees: {}", e),
                )
            })?;

        let wt = worktrees
            .iter()
            .find(|wt| wt.branch == branch)
            .ok_or_else(|| {
                error_response(
                    StatusCode::NOT_FOUND,
                    format!("Worktree '{}' not found", branch),
                )
            })?;

        Ok(wt.path.clone())
    } else {
        Err(error_response(
            StatusCode::BAD_REQUEST,
            "Invalid root parameter. Use 'base' or 'worktree:<branch>'",
        ))
    }
}

/// Validate a relative path: reject path traversal attempts.
#[allow(clippy::result_large_err)]
pub(super) fn validate_relative_path(path: &str) -> Result<(), Response> {
    if path.contains("..") {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Path traversal is not allowed",
        ));
    }
    // Also reject absolute paths
    if path.starts_with('/') || path.starts_with('\\') {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "Absolute paths are not allowed",
        ));
    }
    Ok(())
}

/// Build a recursive file tree for the given directory.
pub(super) fn build_file_tree(
    dir: &std::path::Path,
    root: &std::path::Path,
) -> std::io::Result<Vec<FileTreeEntry>> {
    let mut entries = Vec::new();
    let mut dir_entries: Vec<_> = std::fs::read_dir(dir)?.flatten().collect();
    dir_entries.sort_by_key(|e| e.file_name());

    for entry in dir_entries {
        let file_name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files starting with '.' except specific ones we want to show
        if file_name.starts_with('.') && file_name != ".cflx.jsonc" {
            // Skip .git but not other dot-dirs that might be relevant
            if EXCLUDED_DIRS.contains(&file_name.as_str()) {
                continue;
            }
        }

        let path = entry.path();
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            // Skip excluded directories
            if EXCLUDED_DIRS.contains(&file_name.as_str()) {
                continue;
            }

            let children = build_file_tree(&path, root)?;
            entries.push(FileTreeEntry {
                name: file_name,
                path: relative_path,
                entry_type: "directory".to_string(),
                children: Some(children),
            });
        } else if file_type.is_file() {
            entries.push(FileTreeEntry {
                name: file_name,
                path: relative_path,
                entry_type: "file".to_string(),
                children: None,
            });
        }
    }

    Ok(entries)
}

/// Detect if a file is binary by checking for NUL bytes in the first 8KB.
pub(super) fn is_binary_file(path: &std::path::Path) -> std::io::Result<bool> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 8192];
    let n = file.read(&mut buf)?;
    Ok(buf[..n].contains(&0))
}

/// GET /api/v1/projects/:id/files/tree - list file tree
pub async fn get_file_tree(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<FileTreeQuery>,
) -> Response {
    let root_path = match resolve_file_root(&state, &project_id, &query.root).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    match build_file_tree(&root_path, &root_path) {
        Ok(tree) => (StatusCode::OK, Json(tree)).into_response(),
        Err(e) => {
            error!(
                project_id = %project_id,
                error = %e,
                "Failed to build file tree"
            );
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to list files: {}", e),
            )
        }
    }
}

/// GET /api/v1/projects/:id/files/content - read file content
pub async fn get_file_content(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<FileContentQuery>,
) -> Response {
    // Validate path: reject traversal
    if let Err(resp) = validate_relative_path(&query.path) {
        return resp;
    }

    let root_path = match resolve_file_root(&state, &project_id, &query.root).await {
        Ok(p) => p,
        Err(resp) => return resp,
    };

    let file_path = root_path.join(&query.path);

    // Ensure the resolved path is still within the root (canonicalize both)
    let canonical_root = match root_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return error_response(StatusCode::NOT_FOUND, "Root path not found");
        }
    };
    let canonical_file = match file_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return error_response(StatusCode::NOT_FOUND, "File not found");
        }
    };
    if !canonical_file.starts_with(&canonical_root) {
        return error_response(StatusCode::BAD_REQUEST, "Path traversal is not allowed");
    }

    if !canonical_file.is_file() {
        return error_response(StatusCode::NOT_FOUND, "File not found");
    }

    // Get file size
    let metadata = match std::fs::metadata(&canonical_file) {
        Ok(m) => m,
        Err(_) => {
            return error_response(StatusCode::NOT_FOUND, "File not found");
        }
    };
    let size = metadata.len();

    // Check if binary
    match is_binary_file(&canonical_file) {
        Ok(true) => {
            return (
                StatusCode::OK,
                Json(FileContentResponse {
                    path: query.path,
                    content: None,
                    size,
                    truncated: false,
                    binary: true,
                }),
            )
                .into_response();
        }
        Ok(false) => {}
        Err(_) => {
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file");
        }
    }

    // Read content (truncate if too large)
    let truncated = size > MAX_FILE_CONTENT_SIZE;
    let content = if truncated {
        use std::io::Read;
        let mut file = match std::fs::File::open(&canonical_file) {
            Ok(f) => f,
            Err(_) => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file");
            }
        };
        let mut buf = vec![0u8; MAX_FILE_CONTENT_SIZE as usize];
        let n = match file.read(&mut buf) {
            Ok(n) => n,
            Err(_) => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file");
            }
        };
        buf.truncate(n);
        String::from_utf8_lossy(&buf).to_string()
    } else {
        match std::fs::read_to_string(&canonical_file) {
            Ok(s) => s,
            Err(_) => {
                return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to read file");
            }
        }
    };

    (
        StatusCode::OK,
        Json(FileContentResponse {
            path: query.path,
            content: Some(content),
            size,
            truncated,
            binary: false,
        }),
    )
        .into_response()
}
