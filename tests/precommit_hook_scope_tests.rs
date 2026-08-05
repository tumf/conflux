//! Regression coverage for the commit-time selection of the Rust hooks in
//! `.pre-commit-config.yaml`.
//!
//! These assertions read the tracked configuration file, so they are
//! integration evidence rather than unit tests. They exist because the
//! narrowing here is *selection only*: the hook commands must keep validating
//! the whole workspace once selected, and a future edit that reintroduces
//! `always_run`, drops the shared selector, or starts passing staged filenames
//! would silently change what a commit verifies.

use std::fs;
use std::path::Path;

use regex::Regex;
use serde_yaml::Value;

/// The one selector both Rust hooks must share.
const RUST_SELECTOR: &str = r"^(src|tests)/.*\.rs$|^Cargo\.(toml|lock)$|^build\.rs$";

const RUSTFMT_ENTRY: &str = "cargo fmt --all";
const CLIPPY_ENTRY: &str = "cargo clippy --locked --all-targets --all-features -- -D warnings";

fn config() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(".pre-commit-config.yaml");
    let raw = fs::read_to_string(&path).expect(".pre-commit-config.yaml");
    serde_yaml::from_str(&raw).expect(".pre-commit-config.yaml must be valid YAML")
}

/// Every hook declared by the `local` repo, in declaration order.
fn local_hooks(config: &Value) -> Vec<&Value> {
    config["repos"]
        .as_sequence()
        .expect("repos must be a sequence")
        .iter()
        .filter(|repo| repo["repo"].as_str() == Some("local"))
        .flat_map(|repo| {
            repo["hooks"]
                .as_sequence()
                .expect("local repo must declare hooks")
                .iter()
        })
        .collect()
}

fn local_hook<'a>(config: &'a Value, id: &str) -> &'a Value {
    local_hooks(config)
        .into_iter()
        .find(|hook| hook["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("local hook {id:?} must exist"))
}

#[test]
fn rust_hooks_share_the_exact_path_selector() {
    let config = config();

    for id in ["rustfmt", "clippy"] {
        let hook = local_hook(&config, id);
        assert_eq!(
            hook["files"].as_str(),
            Some(RUST_SELECTOR),
            "hook {id:?} must use the shared Rust selector verbatim"
        );
    }
}

#[test]
fn rust_hooks_do_not_run_unconditionally() {
    let config = config();

    for id in ["rustfmt", "clippy"] {
        let hook = local_hook(&config, id);
        assert!(
            hook.get("always_run").is_none(),
            "hook {id:?} must not declare always_run; path selection is the point of this config"
        );
        assert!(
            hook.get("exclude").is_none(),
            "hook {id:?} must not narrow the shared selector with exclude"
        );
    }
}

#[test]
fn rust_hooks_keep_their_full_workspace_commands() {
    let config = config();

    for (id, entry) in [("rustfmt", RUSTFMT_ENTRY), ("clippy", CLIPPY_ENTRY)] {
        let hook = local_hook(&config, id);
        assert_eq!(
            hook["entry"].as_str(),
            Some(entry),
            "hook {id:?} must keep validating the full Rust workspace"
        );
        // Selection is narrowed; the command scope is not. A hook that started
        // receiving staged filenames would check only those files.
        assert_eq!(
            hook["pass_filenames"].as_bool(),
            Some(false),
            "hook {id:?} must not receive staged filenames"
        );
    }
}

#[test]
fn selector_matches_rust_impacting_paths_only() {
    let selector = Regex::new(RUST_SELECTOR).expect("selector must be a valid regex");

    for selected in [
        "src/main.rs",
        "src/web/openapi.rs",
        "tests/precommit_hook_scope_tests.rs",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
    ] {
        assert!(
            selector.is_match(selected),
            "{selected:?} can affect Rust compilation and must select the Rust hooks"
        );
    }

    for skipped in [
        "openspec/changes/some-change/proposal.md",
        "openspec/CONSTITUTION.md",
        "README.md",
        "docs/guides/DEVELOPMENT.md",
        "CONTRIBUTING.md",
        "Makefile",
        "web/app.js",
        "skills/cflx-apply/SKILL.md",
        // Only the root manifests and root build script are Rust build inputs
        // for this workspace; look-alike paths elsewhere must not select.
        "tests/web/package.json",
        "docs/Cargo.toml",
        "scripts/build.rs",
        "src/notes.md",
    ] {
        assert!(
            !selector.is_match(skipped),
            "{skipped:?} cannot affect Rust compilation and must not select the Rust hooks"
        );
    }
}

#[test]
fn non_rust_local_hooks_keep_running_unconditionally() {
    let config = config();

    for id in ["beads-pre-commit", "openapi-contract"] {
        let hook = local_hook(&config, id);
        assert_eq!(
            hook["always_run"].as_bool(),
            Some(true),
            "non-Rust hook {id:?} must retain its existing selection behavior"
        );
        assert!(
            hook.get("files").is_none(),
            "non-Rust hook {id:?} must not gain a path selector"
        );
    }
}

#[test]
fn development_docs_state_the_selection_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let contributing =
        fs::read_to_string(root.join("CONTRIBUTING.md")).expect("CONTRIBUTING.md must exist");
    let development = fs::read_to_string(root.join("docs/guides/DEVELOPMENT.md"))
        .expect("docs/guides/DEVELOPMENT.md must exist");

    for (name, doc) in [
        ("CONTRIBUTING.md", &contributing),
        ("docs/guides/DEVELOPMENT.md", &development),
    ] {
        assert!(
            doc.contains("path-scoped"),
            "{name} must describe commit-time Rust checks as path-scoped"
        );
        assert!(
            doc.contains("proposal-only"),
            "{name} must state that proposal-only commits skip the Rust hooks"
        );
        assert!(
            doc.contains("make check"),
            "{name} must point at the explicit full validation command"
        );
        // A manual `prek run rustfmt` with no file arguments now selects
        // nothing, so the documented examples must pass `--all-files`.
        assert!(
            doc.contains("--all-files"),
            "{name} must show the explicit full-run form of the Rust hooks"
        );
        assert!(
            !doc.contains("prek run rustfmt clippy\n"),
            "{name} must not show a bare `prek run rustfmt clippy`, which selects no files"
        );
    }
}
