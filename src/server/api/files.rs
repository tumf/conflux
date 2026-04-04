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

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Method, Request, StatusCode};
    use tempfile::TempDir;
    use tower::ServiceExt;

    use super::*;
    use crate::server::api::build_router;
    use crate::server::api::test_support::{create_local_git_repo, make_router, make_state};

    #[test]
    fn test_validate_relative_path_rejects_traversal() {
        assert!(validate_relative_path("../etc/passwd").is_err());
        assert!(validate_relative_path("foo/../bar").is_err());
        assert!(validate_relative_path("..").is_err());
    }

    #[test]
    fn test_validate_relative_path_rejects_absolute() {
        assert!(validate_relative_path("/etc/passwd").is_err());
        assert!(validate_relative_path("\\windows\\system32").is_err());
    }

    #[test]
    fn test_validate_relative_path_accepts_valid() {
        assert!(validate_relative_path("src/main.rs").is_ok());
        assert!(validate_relative_path("openspec/changes/add-feature/proposal.md").is_ok());
        assert!(validate_relative_path("Cargo.toml").is_ok());
    }

    #[test]
    fn test_build_file_tree_excludes_dirs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/foo")).unwrap();
        std::fs::create_dir_all(root.join("target/debug")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();

        let tree = build_file_tree(root, root).unwrap();
        let names: Vec<&str> = tree.iter().map(|e| e.name.as_str()).collect();

        assert!(!names.contains(&".git"), "should exclude .git");
        assert!(
            !names.contains(&"node_modules"),
            "should exclude node_modules"
        );
        assert!(!names.contains(&"target"), "should exclude target");
        assert!(names.contains(&"src"), "should include src");
        assert!(names.contains(&"Cargo.toml"), "should include Cargo.toml");
    }

    #[test]
    fn test_build_file_tree_recursive() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();

        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/c.txt"), "hello").unwrap();
        std::fs::write(root.join("a/d.txt"), "world").unwrap();

        let tree = build_file_tree(root, root).unwrap();
        assert_eq!(tree.len(), 1);
        let a = &tree[0];
        assert_eq!(a.name, "a");
        assert_eq!(a.entry_type, "directory");

        let children_a = a.children.as_ref().unwrap();
        assert_eq!(children_a.len(), 2);

        let b = children_a.iter().find(|e| e.name == "b").unwrap();
        assert_eq!(b.entry_type, "directory");
        let children_b = b.children.as_ref().unwrap();
        assert_eq!(children_b.len(), 1);
        assert_eq!(children_b[0].name, "c.txt");
        assert_eq!(children_b[0].entry_type, "file");
    }

    #[test]
    fn test_is_binary_file_text() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("text.txt");
        std::fs::write(&path, "Hello, world!").unwrap();
        assert!(!is_binary_file(&path).unwrap());
    }

    #[test]
    fn test_is_binary_file_binary() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("binary.bin");
        std::fs::write(&path, b"\x00\x01\x02\x03").unwrap();
        assert!(is_binary_file(&path).unwrap());
    }

    #[tokio::test]
    async fn test_get_file_tree_project_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/nonexistent/files/tree")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_file_content_project_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, None);

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/nonexistent/files/content?path=foo.txt")
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_file_content_rejects_path_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let state = make_state(&temp_dir, None);
        let entry = state
            .registry
            .write()
            .await
            .add("https://github.com/foo/bar".to_string(), "main".to_string())
            .unwrap();

        let router = build_router(state.clone());

        let req = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/api/v1/projects/{}/files/content?path=../../../etc/passwd",
                entry.id
            ))
            .body(Body::empty())
            .unwrap();

        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_file_tree_with_real_project() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = make_router(&temp_dir, None);

        let add_body = serde_json::json!({
            "remote_url": remote_url,
            "branch": "main"
        });

        let add_req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects")
            .header("Content-Type", "application/json")
            .body(Body::from(add_body.to_string()))
            .unwrap();

        let add_resp = router.clone().oneshot(add_req).await.unwrap();
        assert_eq!(add_resp.status(), StatusCode::CREATED);

        let body_bytes = axum::body::to_bytes(add_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let project_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let project_id = project_json["id"].as_str().unwrap();

        let tree_req = Request::builder()
            .method(Method::GET)
            .uri(format!("/api/v1/projects/{}/files/tree", project_id))
            .body(Body::empty())
            .unwrap();

        let tree_resp = router.clone().oneshot(tree_req).await.unwrap();
        assert_eq!(tree_resp.status(), StatusCode::OK);

        let tree_body = axum::body::to_bytes(tree_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let tree: Vec<serde_json::Value> = serde_json::from_slice(&tree_body).unwrap();
        let names: Vec<&str> = tree.iter().filter_map(|e| e["name"].as_str()).collect();
        assert!(
            names.contains(&"README.md"),
            "File tree should contain README.md, got: {:?}",
            names
        );
    }

    #[tokio::test]
    async fn test_get_file_content_with_real_project() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = make_router(&temp_dir, None);

        let add_body = serde_json::json!({
            "remote_url": remote_url,
            "branch": "main"
        });

        let add_req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects")
            .header("Content-Type", "application/json")
            .body(Body::from(add_body.to_string()))
            .unwrap();

        let add_resp = router.clone().oneshot(add_req).await.unwrap();
        assert_eq!(add_resp.status(), StatusCode::CREATED);

        let body_bytes = axum::body::to_bytes(add_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let project_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let project_id = project_json["id"].as_str().unwrap();

        let content_req = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/api/v1/projects/{}/files/content?path=README.md",
                project_id
            ))
            .body(Body::empty())
            .unwrap();

        let content_resp = router.oneshot(content_req).await.unwrap();
        assert_eq!(content_resp.status(), StatusCode::OK);

        let content_body = axum::body::to_bytes(content_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let content: serde_json::Value = serde_json::from_slice(&content_body).unwrap();
        assert_eq!(content["path"], "README.md");
        assert_eq!(content["binary"], false);
        assert_eq!(content["truncated"], false);
        assert!(content["content"].is_string());
        assert_eq!(content["content"].as_str().unwrap(), "hello");
    }

    #[tokio::test]
    async fn test_get_file_content_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let origin = create_local_git_repo(temp_dir.path());
        let remote_url = format!("file://{}", origin.to_str().unwrap());

        let router = make_router(&temp_dir, None);

        let add_body = serde_json::json!({
            "remote_url": remote_url,
            "branch": "main"
        });
        let add_req = Request::builder()
            .method(Method::POST)
            .uri("/api/v1/projects")
            .header("Content-Type", "application/json")
            .body(Body::from(add_body.to_string()))
            .unwrap();
        let add_resp = router.clone().oneshot(add_req).await.unwrap();
        assert_eq!(add_resp.status(), StatusCode::CREATED);

        let body_bytes = axum::body::to_bytes(add_resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let project_json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        let project_id = project_json["id"].as_str().unwrap();

        let content_req = Request::builder()
            .method(Method::GET)
            .uri(format!(
                "/api/v1/projects/{}/files/content?path=nonexistent.txt",
                project_id
            ))
            .body(Body::empty())
            .unwrap();

        let content_resp = router.oneshot(content_req).await.unwrap();
        assert_eq!(content_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_file_api_requires_auth() {
        let temp_dir = TempDir::new().unwrap();
        let router = make_router(&temp_dir, Some("secret-token"));

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/some-id/files/tree")
            .body(Body::empty())
            .unwrap();
        let resp = router.clone().oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "File tree endpoint should require authentication"
        );

        let req = Request::builder()
            .method(Method::GET)
            .uri("/api/v1/projects/some-id/files/content?path=foo.txt")
            .body(Body::empty())
            .unwrap();
        let resp = router.oneshot(req).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "File content endpoint should require authentication"
        );
    }
}
