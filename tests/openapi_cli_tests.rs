//! `cflx openapi` as an operator actually invokes it.
//!
//! The contract is generated rather than tracked, so this command *is* the
//! export path. What matters to a consumer is therefore behavioural rather than
//! structural: the process must exit successfully with nothing but the schema on
//! stdout, it must work where no Git repository exists, it must not take the
//! repository lock or open a listener on its way through, and a build that
//! cannot produce a complete document must refuse rather than emit a partial
//! one.
//!
//! Both halves live here because they are the same command seen under the two
//! feature configurations, and each half only compiles under the configuration
//! it describes:
//!
//! ```text
//! cargo test --features web-monitoring --test openapi_cli_tests
//! cargo test --no-default-features --test openapi_cli_tests
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch directory that is deliberately *not* inside a Git repository.
///
/// `cflx openapi` must not depend on repository context, and the surest way to
/// prove that is to run it where there is none.
fn scratch_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "cflx-openapi-cli-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory is creatable");
    assert!(
        !dir.join(".git").exists(),
        "the scratch directory must not be a repository"
    );
    dir
}

fn cflx_openapi(workdir: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cflx"))
        .arg("openapi")
        .current_dir(workdir)
        // A stray user config must not be able to change what the export emits.
        .env("HOME", workdir.join("home"))
        .env("XDG_STATE_HOME", workdir.join("state"))
        .output()
        .expect("the cflx binary under test is runnable")
}

// ============================================================================
// The export path
// ============================================================================

#[cfg(feature = "web-monitoring")]
mod available {
    use super::*;

    use serde_json::Value;

    /// The whole point of the command: redirect it and you have a valid,
    /// standalone OpenAPI 3.1 document, produced without a repository.
    #[test]
    fn exports_a_complete_document_outside_a_git_repository() {
        let dir = scratch_dir("export");
        let output = cflx_openapi(&dir);

        assert!(
            output.status.success(),
            "cflx openapi must succeed outside a repository: status={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );

        let stdout = String::from_utf8(output.stdout).expect("the schema is UTF-8");
        let document: Value =
            serde_yaml::from_str(&stdout).expect("stdout must parse as a YAML document on its own");
        assert_eq!(
            document["openapi"], "3.1.0",
            "the exported document must declare the OpenAPI version a generator reads"
        );
        assert!(
            document["paths"]["/api/v2/state"].is_object(),
            "the export must carry the served route surface, not a stub"
        );
        // An agent reads the exported contract to learn that execution
        // observability exists at all, so the export and the live endpoint must
        // agree about it rather than only the live one carrying it.
        assert!(
            document["paths"]["/api/v2/execution-status"].is_object(),
            "the export must carry the execution-status resource"
        );
        assert!(
            document["components"]["schemas"]["ExecutionPhase"]["enum"]
                .as_array()
                .is_some_and(|values| values.iter().any(|value| value == "acceptance")),
            "the export must carry the closed phase vocabulary"
        );
        assert!(
            document["components"]["schemas"]["CommandResult"].is_object(),
            "the export must carry the typed command-result contract"
        );

        std::fs::remove_dir_all(&dir).expect("scratch dir is removable");
    }

    /// Diagnostics on stdout would corrupt the redirected file, so a successful
    /// run must say nothing anywhere else.
    #[test]
    fn says_nothing_on_standard_error_when_it_succeeds() {
        let dir = scratch_dir("quiet");
        let output = cflx_openapi(&dir);

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr),
            "",
            "a successful export must not mix diagnostics into the operator's terminal"
        );

        std::fs::remove_dir_all(&dir).expect("scratch dir is removable");
    }

    /// Byte-for-byte the same function the live endpoint serves.
    ///
    /// The contract test proves CLI/router parity through the router; this
    /// proves the *process* emits those bytes unaltered — no logging preamble,
    /// no trailing newline drift, nothing added by the CLI layer.
    #[test]
    fn emits_exactly_the_generated_document() {
        let dir = scratch_dir("bytes");
        let output = cflx_openapi(&dir);

        assert_eq!(
            String::from_utf8(output.stdout).expect("the schema is UTF-8"),
            conflux::web::openapi::document_yaml(),
            "the CLI must emit the generated document unmodified"
        );

        std::fs::remove_dir_all(&dir).expect("scratch dir is removable");
    }

    /// A read-only export has no business claiming the repository or opening a
    /// listener, and running it inside a repository is the only place that can
    /// be observed. Both side effects leave a file in the Git common directory,
    /// so their absence there is the proof.
    #[test]
    fn takes_no_repository_lock_and_opens_no_socket() {
        let dir = scratch_dir("in-repo");
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(&dir)
                .env("HOME", dir.join("home"))
                .status()
                .expect("git is available");
            assert!(status.success(), "git {args:?} failed");
        };
        git(&["init", "--quiet"]);

        let output = cflx_openapi(&dir);
        assert!(
            output.status.success(),
            "cflx openapi must succeed inside a repository too: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );

        let git_dir = dir.join(".git");
        assert!(
            !git_dir.join("cflx-orchestration.lock").exists(),
            "a read-only export must not take the repository orchestration lock"
        );
        assert!(
            !git_dir.join("cflx-api.sock").exists(),
            "a read-only export must not bind the local API socket"
        );

        std::fs::remove_dir_all(&dir).expect("scratch dir is removable");
    }
}

// ============================================================================
// The unavailable path
// ============================================================================

#[cfg(not(feature = "web-monitoring"))]
mod unavailable {
    use super::*;

    /// A build without the feature that declares the contract has no complete
    /// document to give. Emitting a partial one would be the worse failure: a
    /// generated client believes whatever it is handed, so the command has to
    /// fail loudly and write no schema at all.
    #[test]
    fn refuses_clearly_and_writes_no_schema() {
        let dir = scratch_dir("unavailable");
        let output = cflx_openapi(&dir);

        assert!(
            !output.status.success(),
            "an unavailable contract must exit non-zero"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "",
            "stdout must carry no partial schema when the document cannot be produced"
        );

        let stderr = String::from_utf8_lossy(&output.stderr).to_lowercase();
        assert!(
            stderr.contains("openapi") && stderr.contains("unavailable"),
            "stderr must explain that OpenAPI support is unavailable, got: {stderr}"
        );
        assert!(
            stderr.contains("web-monitoring"),
            "stderr must name the feature that restores the command, got: {stderr}"
        );

        std::fs::remove_dir_all(&dir).expect("scratch dir is removable");
    }
}
