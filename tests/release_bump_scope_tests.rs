#![cfg(unix)]

//! Integration coverage for the scoped main/master release path in
//! `scripts/bump.sh`.
//!
//! These tests intentionally exercise real boundaries: temporary Git
//! repositories, a local bare origin, real Git hooks, and a controlled fake
//! `cargo` on `PATH`. They are integration evidence, not unit tests, and they
//! never touch the network or the developer's own repository.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const RELEASE_COMMIT_PREFIX: &str = "chore(release): release v";

const FAKE_CARGO: &str = r#"#!/usr/bin/env bash
set -uo pipefail

printf '%s\n' "$*" >>"$FAKE_CARGO_LOG"

mode="${FAKE_CARGO_MODE:-normal}"

if [[ "${1:-}" == "generate-lockfile" ]]; then
	case "$mode" in
	fail)
		echo "fake cargo: generate-lockfile failed" >&2
		exit 1
		;;
	revert)
		git checkout -- Cargo.toml
		if [[ -f docs/openapi.yaml ]]; then
			git checkout -- docs/openapi.yaml
		fi
		exit 0
		;;
	esac

	version=$(grep -E '^version[[:space:]]*=' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
	printf '# fake lockfile\n[[package]]\nname = "fixture"\nversion = "%s"\n' "$version" >Cargo.lock
	exit 0
fi

exit 0
"#;

/// Rejects creation of any tag ref, simulating a tag failure after the release
/// commit has already been created.
const REJECT_TAGS_HOOK: &str = r#"#!/usr/bin/env bash
if [[ "${1:-}" != "prepared" ]]; then
	exit 0
fi
while read -r _old _new ref; do
	if [[ "$ref" == refs/tags/* ]]; then
		echo "reference-transaction hook: refusing $ref" >&2
		exit 1
	fi
done
exit 0
"#;

const FAILING_HOOK: &str = "#!/usr/bin/env bash\necho \"hook rejected\" >&2\nexit 1\n";

#[derive(Debug, Default)]
struct WorkingState {
    staged: Vec<String>,
    unstaged: Vec<String>,
    untracked: Vec<String>,
}

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    repo: PathBuf,
    origin: PathBuf,
    bin: PathBuf,
    cargo_log: PathBuf,
    gitconfig: PathBuf,
}

impl Fixture {
    /// Each test gets its own repository, bare origin, and fake `cargo`, so
    /// tests stay independent and can run in parallel.
    fn new(with_openapi: bool) -> Self {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().to_path_buf();
        let repo = root.join("repo");
        let origin = root.join("origin.git");
        let bin = root.join("bin");
        let cargo_log = root.join("cargo.log");
        let gitconfig = root.join("gitconfig");

        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&bin).unwrap();
        fs::write(&gitconfig, "").unwrap();
        fs::write(&cargo_log, "").unwrap();

        let fake_cargo = bin.join("cargo");
        fs::write(&fake_cargo, FAKE_CARGO).unwrap();
        fs::set_permissions(&fake_cargo, fs::Permissions::from_mode(0o755)).unwrap();

        let fixture = Fixture {
            _temp: temp,
            root,
            repo,
            origin,
            bin,
            cargo_log,
            gitconfig,
        };

        fixture.init_repo(with_openapi);
        fixture
    }

    fn init_repo(&self, with_openapi: bool) {
        // Identity comes from the environment and signing config cannot leak in
        // (GIT_CONFIG_GLOBAL points at an empty file and system config is off),
        // so the fixture needs no `git config` calls here.
        self.git_ok(&["init", "-q", "-b", "main"]);

        self.write(
            "Cargo.toml",
            "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        );
        self.write(
            "Cargo.lock",
            "# fake lockfile\n[[package]]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
        );
        self.write("README.md", "# fixture\n");
        self.write("notes.md", "tracked unrelated notes\n");
        if with_openapi {
            self.write(
                "docs/openapi.yaml",
                "openapi: 3.0.3\ninfo:\n  title: Fixture API\n  version: 0.1.0\n",
            );
        }

        self.git_ok(&["add", "-A"]);
        self.git_ok(&["commit", "-q", "-m", "init"]);

        self.git_ok(&["init", "--bare", "-q", self.origin.to_str().unwrap()]);
        // Registering the remote by config avoids another process spawn.
        let mut config = fs::read_to_string(self.repo.join(".git").join("config")).unwrap();
        config.push_str(&format!(
            "[remote \"origin\"]\n\turl = {}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n",
            self.origin.display()
        ));
        fs::write(self.repo.join(".git").join("config"), config).unwrap();
        self.git_ok(&["push", "-q", "origin", "main"]);
    }

    fn env_pairs(&self) -> Vec<(String, String)> {
        let path = format!(
            "{}:{}",
            self.bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        vec![
            ("PATH".to_string(), path),
            ("HOME".to_string(), self.root.display().to_string()),
            (
                "GIT_CONFIG_GLOBAL".to_string(),
                self.gitconfig.display().to_string(),
            ),
            ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
            ("GIT_TERMINAL_PROMPT".to_string(), "0".to_string()),
            ("GIT_AUTHOR_NAME".to_string(), "Release Test".to_string()),
            (
                "GIT_AUTHOR_EMAIL".to_string(),
                "release-test@example.com".to_string(),
            ),
            (
                "GIT_AUTHOR_DATE".to_string(),
                "2026-01-01T00:00:00+00:00".to_string(),
            ),
            ("GIT_COMMITTER_NAME".to_string(), "Release Test".to_string()),
            (
                "GIT_COMMITTER_EMAIL".to_string(),
                "release-test@example.com".to_string(),
            ),
            (
                "GIT_COMMITTER_DATE".to_string(),
                "2026-01-01T00:00:00+00:00".to_string(),
            ),
            (
                "FAKE_CARGO_LOG".to_string(),
                self.cargo_log.display().to_string(),
            ),
        ]
    }

    fn git_raw(&self, dir: &Path, args: &[&str]) -> Output {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .envs(self.env_pairs())
            .output()
            .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"))
    }

    fn git(&self, args: &[&str]) -> Output {
        self.git_raw(&self.repo, args)
    }

    fn git_ok(&self, args: &[&str]) -> String {
        let out = self.git(args);
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            describe(&out)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    fn origin_git(&self, args: &[&str]) -> Output {
        self.git_raw(&self.origin, args)
    }

    /// Runs `scripts/bump.sh` from the repository under test.
    fn bump(&self, args: &[&str]) -> Output {
        self.bump_with_env(args, &[])
    }

    fn bump_with_env(&self, args: &[&str], extra: &[(&str, &str)]) -> Output {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/bump.sh");
        let mut cmd = Command::new("bash");
        cmd.arg(&script)
            .args(args)
            .current_dir(&self.repo)
            .env_clear()
            .envs(self.env_pairs());
        for (key, value) in extra {
            cmd.env(key, value);
        }
        cmd.output().expect("failed to run bump.sh")
    }

    fn write(&self, rel: &str, contents: &str) {
        let path = self.repo.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.repo.join(rel)).unwrap()
    }

    fn exists(&self, rel: &str) -> bool {
        self.repo.join(rel).exists()
    }

    fn head(&self) -> String {
        self.git_ok(&["rev-parse", "HEAD"])
    }

    fn head_subject(&self) -> String {
        self.git_ok(&["log", "-1", "--pretty=%s"])
    }

    fn commit_count(&self) -> usize {
        self.git_ok(&["rev-list", "--count", "HEAD"])
            .parse()
            .unwrap()
    }

    /// Paths contained in the given commit, sorted.
    fn commit_files(&self, rev: &str) -> Vec<String> {
        let mut files: Vec<String> = self
            .git_ok(&["show", "--pretty=format:", "--name-only", rev])
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        files.sort();
        files
    }

    fn tags(&self) -> Vec<String> {
        self.git_ok(&["tag", "--list"])
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    }

    /// Index, worktree, and untracked state from a single `git status` call.
    fn status(&self) -> WorkingState {
        // Read stdout untrimmed: the two status columns are leading whitespace.
        let out = self.git(&["status", "--porcelain"]);
        assert!(
            out.status.success(),
            "git status failed: {}",
            describe(&out)
        );
        let raw = String::from_utf8_lossy(&out.stdout).to_string();
        let mut state = WorkingState::default();
        for line in raw.lines() {
            if line.len() < 4 {
                continue;
            }
            let (code, path) = line.split_at(3);
            let path = path.trim().to_string();
            let mut code = code.chars();
            let index = code.next().unwrap_or(' ');
            let worktree = code.next().unwrap_or(' ');
            if index == '?' {
                state.untracked.push(path);
                continue;
            }
            if index != ' ' {
                state.staged.push(path.clone());
            }
            if worktree != ' ' {
                state.unstaged.push(path);
            }
        }
        state.staged.sort();
        state.unstaged.sort();
        state.untracked.sort();
        state
    }

    /// Every ref published to the bare origin, from a single `git show-ref`.
    /// (`show-ref` exits non-zero when the origin has no refs at all.)
    fn origin_refs(&self) -> BTreeMap<String, String> {
        let out = self.origin_git(&["show-ref"]);
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| line.split_once(' '))
            .map(|(sha, name)| (name.trim().to_string(), sha.trim().to_string()))
            .collect()
    }

    fn origin_rev(&self, refname: &str) -> Option<String> {
        self.origin_refs().get(refname).cloned()
    }

    fn install_hook(&self, name: &str, body: &str) {
        let hooks = self.repo.join(".git").join("hooks");
        fs::create_dir_all(&hooks).unwrap();
        let path = hooks.join(name);
        fs::write(&path, body).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn remove_hook(&self, name: &str) {
        let _ = fs::remove_file(self.repo.join(".git").join("hooks").join(name));
    }

    fn cargo_log(&self) -> String {
        fs::read_to_string(&self.cargo_log).unwrap_or_default()
    }

    /// Creates unrelated staged, unstaged, and untracked work that a release
    /// must never absorb or disturb.
    fn add_unrelated_work(&self) {
        self.write("staged.md", "staged unrelated work\n");
        self.git_ok(&["add", "staged.md"]);
        self.write("notes.md", "tracked unrelated notes\nmodified locally\n");
        self.write("untracked.md", "untracked unrelated work\n");
    }

    fn assert_unrelated_work_preserved(&self) {
        let state = self.status();
        assert!(
            state.staged.contains(&"staged.md".to_string()),
            "staged unrelated file lost its staged state: {state:?}"
        );
        assert!(
            state.unstaged.contains(&"notes.md".to_string()),
            "unstaged unrelated modification was lost: {state:?}"
        );
        assert!(
            state.untracked.contains(&"untracked.md".to_string()),
            "untracked unrelated file was lost: {state:?}"
        );
        assert_eq!(
            self.read("notes.md"),
            "tracked unrelated notes\nmodified locally\n"
        );
        assert_eq!(self.read("untracked.md"), "untracked unrelated work\n");
    }

    fn manifest_version(&self) -> String {
        self.read("Cargo.toml")
            .lines()
            .find(|l| l.trim_start().starts_with("version"))
            .and_then(|l| l.split('"').nth(1).map(|s| s.to_string()))
            .expect("version in Cargo.toml")
    }
}

fn describe(out: &Output) -> String {
    format!(
        "status={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

fn stdout_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn stderr_of(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

fn assert_success(out: &Output) {
    assert!(out.status.success(), "bump.sh failed: {}", describe(out));
}

fn assert_failure(out: &Output) {
    assert!(
        !out.status.success(),
        "bump.sh unexpectedly succeeded: {}",
        describe(out)
    );
}

// --- Scoped commit contents -------------------------------------------------

#[test]
fn release_commit_contains_only_owned_paths_and_preserves_unrelated_work() {
    let fx = Fixture::new(true);
    fx.add_unrelated_work();
    let base_head = fx.head();

    let out = fx.bump(&["patch"]);
    assert_success(&out);

    assert_eq!(fx.head_subject(), format!("{RELEASE_COMMIT_PREFIX}0.1.1"));
    assert_eq!(
        fx.commit_files("HEAD"),
        vec![
            "Cargo.lock".to_string(),
            "Cargo.toml".to_string(),
            "docs/openapi.yaml".to_string()
        ],
        "release commit must contain only release-owned artifacts"
    );
    assert_eq!(fx.commit_count(), 2, "exactly one release commit is added");
    assert_ne!(fx.head(), base_head);

    assert_eq!(fx.manifest_version(), "0.1.1");
    assert!(fx.read("Cargo.lock").contains("version = \"0.1.1\""));
    assert!(fx.read("docs/openapi.yaml").contains("version: 0.1.1"));

    fx.assert_unrelated_work_preserved();

    // Annotated tag on the scoped release commit.
    assert_eq!(fx.tags(), vec!["v0.1.1".to_string()]);
    assert_eq!(
        fx.git_ok(&["cat-file", "-t", "v0.1.1"]),
        "tag",
        "release tag must be annotated"
    );
    assert_eq!(fx.git_ok(&["rev-list", "-n", "1", "v0.1.1"]), fx.head());
    assert!(fx
        .git_ok(&["tag", "-l", "--format=%(contents)", "v0.1.1"])
        .contains("Release v0.1.1"));

    // Publication reached the local bare origin.
    let published = fx.origin_refs();
    assert_eq!(published.get("refs/heads/main"), Some(&fx.head()));
    assert!(published.contains_key("refs/tags/v0.1.1"));
}

#[test]
fn release_without_openapi_commits_only_manifest_and_lockfile() {
    let fx = Fixture::new(false);

    let out = fx.bump(&["patch"]);
    assert_success(&out);

    assert_eq!(
        fx.commit_files("HEAD"),
        vec!["Cargo.lock".to_string(), "Cargo.toml".to_string()]
    );
    assert!(!fx.exists("docs/openapi.yaml"));
}

#[test]
fn pre_staged_unrelated_entry_stays_out_of_the_release_commit() {
    let fx = Fixture::new(true);
    // An unrelated file staged before the bump must not be swept into the
    // release commit by a pathless commit.
    fx.write(
        "proposal.md",
        "unrelated proposal staged by another session\n",
    );
    fx.git_ok(&["add", "proposal.md"]);
    let staged_blob = fx.git_ok(&["rev-parse", ":proposal.md"]);

    let out = fx.bump(&["patch"]);
    assert_success(&out);

    let committed = fx.commit_files("HEAD");
    assert!(
        !committed.contains(&"proposal.md".to_string()),
        "release commit absorbed an unrelated staged entry: {committed:?}"
    );
    assert_eq!(
        fx.status().staged,
        vec!["proposal.md".to_string()],
        "unrelated entry must remain staged after the release"
    );
    assert_eq!(
        fx.git_ok(&["rev-parse", ":proposal.md"]),
        staged_blob,
        "staged content must be unchanged"
    );
}

// --- Owned-path start-state validation --------------------------------------

#[test]
fn unstaged_owned_path_change_blocks_release_before_mutation() {
    let fx = Fixture::new(true);
    fx.add_unrelated_work();
    let base_head = fx.head();
    fx.write(
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n# local edit\n",
    );

    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    assert!(
        stderr_of(&out).contains("Release-owned paths must match HEAD"),
        "{}",
        describe(&out)
    );

    assert_eq!(fx.head(), base_head, "no release commit may be created");
    assert!(fx.tags().is_empty(), "no release tag may be created");
    assert_eq!(fx.origin_rev("refs/heads/main"), Some(base_head.clone()));
    // Mutation never ran and local state was not cleaned.
    assert_eq!(fx.manifest_version(), "0.1.0");
    assert!(fx.read("Cargo.toml").contains("# local edit"));
    assert!(
        fx.cargo_log().trim().is_empty(),
        "cargo must not run once owned paths are dirty"
    );
    fx.assert_unrelated_work_preserved();
}

#[test]
fn staged_owned_path_change_blocks_release() {
    let fx = Fixture::new(true);
    let base_head = fx.head();
    fx.write(
        "docs/openapi.yaml",
        "openapi: 3.0.3\ninfo:\n  title: Fixture API\n  version: 0.1.0\n  contact: staged\n",
    );
    fx.git_ok(&["add", "docs/openapi.yaml"]);

    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    assert!(stderr_of(&out).contains("Release-owned paths must match HEAD"));
    assert_eq!(fx.head(), base_head);
    assert!(fx.tags().is_empty());
    assert!(fx.read("docs/openapi.yaml").contains("contact: staged"));
}

#[test]
fn deleted_tracked_openapi_blocks_release() {
    let fx = Fixture::new(true);
    let base_head = fx.head();
    // A tracked release-owned artifact deleted from the worktree is still
    // release-owned, so its deletion must be rejected before any mutation.
    fs::remove_file(fx.repo.join("docs/openapi.yaml")).unwrap();

    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    assert!(
        stderr_of(&out).contains("Release-owned paths must match HEAD"),
        "{}",
        describe(&out)
    );
    assert!(
        stderr_of(&out).contains("docs/openapi.yaml"),
        "the deleted owned path must be reported: {}",
        describe(&out)
    );

    assert_eq!(fx.head(), base_head, "no release commit may be created");
    assert_eq!(fx.commit_count(), 1);
    assert!(fx.tags().is_empty(), "no release tag may be created");
    let published = fx.origin_refs();
    assert_eq!(published.get("refs/heads/main"), Some(&base_head));
    assert!(!published.contains_key("refs/tags/v0.1.1"));
    // Mutation never ran and the deletion was not silently restored.
    assert_eq!(fx.manifest_version(), "0.1.0");
    assert!(!fx.exists("docs/openapi.yaml"));
    assert!(
        fx.cargo_log().trim().is_empty(),
        "cargo must not run once owned paths are dirty"
    );
}

#[test]
fn staged_deletion_of_tracked_openapi_blocks_release() {
    let fx = Fixture::new(true);
    let base_head = fx.head();
    fx.git_ok(&["rm", "-q", "docs/openapi.yaml"]);

    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    assert!(stderr_of(&out).contains("Release-owned paths must match HEAD"));

    assert_eq!(fx.head(), base_head);
    assert!(fx.tags().is_empty());
    assert_eq!(fx.origin_rev("refs/heads/main"), Some(base_head));
    assert_eq!(fx.manifest_version(), "0.1.0");
}

#[test]
fn untracked_owned_lockfile_blocks_release() {
    let fx = Fixture::new(false);
    // Simulate a lockfile that exists but was never committed.
    fx.git_ok(&["rm", "-q", "--cached", "Cargo.lock"]);
    fx.git_ok(&["commit", "-q", "-m", "drop lockfile from tracking"]);
    let base_head = fx.head();

    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    assert!(stderr_of(&out).contains("Release-owned paths must match HEAD"));
    assert_eq!(fx.head(), base_head);
    assert!(fx.tags().is_empty());
}

// --- Delta and failure boundaries -------------------------------------------

#[test]
fn missing_owned_delta_exits_without_release_even_when_other_files_are_dirty() {
    let fx = Fixture::new(true);
    fx.add_unrelated_work();
    let base_head = fx.head();

    let out = fx.bump_with_env(&["patch"], &[("FAKE_CARGO_MODE", "revert")]);
    assert_failure(&out);
    assert!(
        stderr_of(&out).contains("No release changes produced"),
        "{}",
        describe(&out)
    );

    assert_eq!(fx.head(), base_head);
    assert!(fx.tags().is_empty());
    let published = fx.origin_refs();
    assert_eq!(published.get("refs/heads/main"), Some(&base_head));
    assert!(!published.contains_key("refs/tags/v0.1.1"));
    fx.assert_unrelated_work_preserved();
}

#[test]
fn generation_failure_blocks_tag_push_and_a_version_advancing_retry() {
    let fx = Fixture::new(true);
    let base_head = fx.head();

    let out = fx.bump_with_env(&["patch"], &[("FAKE_CARGO_MODE", "fail")]);
    assert_failure(&out);

    assert_eq!(fx.head(), base_head, "no release commit");
    assert!(fx.tags().is_empty(), "no release tag");
    assert_eq!(fx.origin_rev("refs/heads/main"), Some(base_head.clone()));
    // Visible local state is left as-is rather than cleaned.
    assert_eq!(fx.manifest_version(), "0.1.1");

    // A retry must refuse while owned paths are dirty, without advancing.
    let retry = fx.bump(&["patch"]);
    assert_failure(&retry);
    assert!(stderr_of(&retry).contains("Release-owned paths must match HEAD"));
    assert_eq!(fx.head(), base_head);
    assert!(fx.tags().is_empty(), "retry must not create any tag");
    assert_eq!(
        fx.manifest_version(),
        "0.1.1",
        "retry must not calculate a later version"
    );
}

#[test]
fn staging_failure_blocks_tag_push_and_a_version_advancing_retry() {
    let fx = Fixture::new(true);
    let base_head = fx.head();
    // A held index lock is the real staging failure this scoping change was
    // written for: `git add` cannot lock the index, so the release must stop
    // before the commit, tag, and push.
    let index_lock = fx.repo.join(".git").join("index.lock");
    fs::write(&index_lock, "").unwrap();

    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    assert!(
        stderr_of(&out).contains("index.lock"),
        "expected a real git staging failure: {}",
        describe(&out)
    );
    fs::remove_file(&index_lock).unwrap();

    assert_eq!(fx.head(), base_head, "no release commit");
    assert_eq!(fx.commit_count(), 1);
    assert!(fx.tags().is_empty(), "no release tag");
    let published = fx.origin_refs();
    assert_eq!(published.get("refs/heads/main"), Some(&base_head));
    assert!(!published.contains_key("refs/tags/v0.1.1"));
    // Visible local state is left as-is rather than cleaned.
    assert_eq!(fx.manifest_version(), "0.1.1", "mutation is left visible");

    // A retry must refuse while owned paths are dirty, without advancing.
    let retry = fx.bump(&["patch"]);
    assert_failure(&retry);
    assert!(stderr_of(&retry).contains("Release-owned paths must match HEAD"));
    assert_eq!(fx.head(), base_head);
    assert!(fx.tags().is_empty(), "retry must not create any tag");
    assert_eq!(fx.origin_rev("refs/heads/main"), Some(base_head));
    assert_eq!(
        fx.manifest_version(),
        "0.1.1",
        "retry must not calculate a later version"
    );
}

#[test]
fn commit_failure_blocks_tag_push_and_a_version_advancing_retry() {
    let fx = Fixture::new(true);
    let base_head = fx.head();
    fx.install_hook("pre-commit", FAILING_HOOK);

    let out = fx.bump(&["patch"]);
    assert_failure(&out);

    assert_eq!(fx.head(), base_head, "no release commit");
    assert!(fx.tags().is_empty(), "no release tag");
    assert_eq!(fx.origin_rev("refs/heads/main"), Some(base_head.clone()));
    assert_eq!(fx.manifest_version(), "0.1.1", "mutation is left visible");

    fx.remove_hook("pre-commit");
    let retry = fx.bump(&["patch"]);
    assert_failure(&retry);
    assert!(stderr_of(&retry).contains("Release-owned paths must match HEAD"));
    assert_eq!(fx.head(), base_head);
    assert!(fx.tags().is_empty());
    assert_eq!(fx.manifest_version(), "0.1.1");
}

#[test]
fn commit_no_verify_env_bypasses_a_failing_pre_commit_hook() {
    let fx = Fixture::new(true);
    fx.install_hook("pre-commit", FAILING_HOOK);

    let out = fx.bump_with_env(&["patch"], &[("OPENSPEC_GIT_COMMIT_NO_VERIFY", "true")]);
    assert_success(&out);

    assert_eq!(fx.head_subject(), format!("{RELEASE_COMMIT_PREFIX}0.1.1"));
    assert_eq!(
        fx.commit_files("HEAD"),
        vec![
            "Cargo.lock".to_string(),
            "Cargo.toml".to_string(),
            "docs/openapi.yaml".to_string()
        ]
    );
    assert_eq!(fx.tags(), vec!["v0.1.1".to_string()]);
    assert_eq!(fx.origin_rev("refs/heads/main"), Some(fx.head()));
}

// --- Recovery ---------------------------------------------------------------

/// Drives a release whose tag creation fails, leaving a release commit at HEAD
/// with no matching tag and no push.
fn release_commit_without_tag(fx: &Fixture) -> String {
    fx.install_hook("reference-transaction", REJECT_TAGS_HOOK);
    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    fx.remove_hook("reference-transaction");

    assert_eq!(
        fx.head_subject(),
        format!("{RELEASE_COMMIT_PREFIX}0.1.1"),
        "expected a release commit at HEAD (requires git >= 2.28 for the \
         reference-transaction hook): {}",
        describe(&out)
    );
    assert!(
        fx.tags().is_empty(),
        "tag creation should have been rejected"
    );
    fx.head()
}

/// Drives a release whose push fails, leaving a tagged release commit that was
/// never published.
fn tagged_release_without_push(fx: &Fixture) -> String {
    fx.install_hook("pre-push", FAILING_HOOK);
    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    fx.remove_hook("pre-push");

    assert_eq!(fx.head_subject(), format!("{RELEASE_COMMIT_PREFIX}0.1.1"));
    assert_eq!(fx.tags(), vec!["v0.1.1".to_string()]);
    fx.head()
}

#[test]
fn release_commit_missing_its_tag_resumes_the_same_version() {
    let fx = Fixture::new(true);
    let release_head = release_commit_without_tag(&fx);
    assert!(fx.origin_rev("refs/tags/v0.1.1").is_none());

    let resume = fx.bump(&["patch"]);
    assert_success(&resume);

    assert_eq!(fx.head(), release_head, "resume must not add a commit");
    assert_eq!(fx.commit_count(), 2);
    assert_eq!(fx.manifest_version(), "0.1.1");
    assert_eq!(fx.tags(), vec!["v0.1.1".to_string()]);
    assert_eq!(fx.git_ok(&["cat-file", "-t", "v0.1.1"]), "tag");
    assert_eq!(fx.git_ok(&["rev-list", "-n", "1", "v0.1.1"]), release_head);
    let published = fx.origin_refs();
    assert_eq!(published.get("refs/heads/main"), Some(&release_head));
    assert!(published.contains_key("refs/tags/v0.1.1"));
}

#[test]
fn tagged_release_missing_its_push_resumes_the_same_version() {
    let fx = Fixture::new(true);
    let release_head = tagged_release_without_push(&fx);
    let before = fx.origin_refs();
    assert!(before.contains_key("refs/heads/main"));
    assert_ne!(before.get("refs/heads/main"), Some(&release_head));
    assert!(!before.contains_key("refs/tags/v0.1.1"));

    let resume = fx.bump(&["patch"]);
    assert_success(&resume);
    assert!(
        stdout_of(&resume).contains("resuming publication"),
        "{}",
        describe(&resume)
    );

    assert_eq!(fx.head(), release_head, "resume must not add a commit");
    assert_eq!(fx.commit_count(), 2);
    assert_eq!(fx.manifest_version(), "0.1.1");
    assert_eq!(
        fx.tags(),
        vec!["v0.1.1".to_string()],
        "resume must not create a later version"
    );
    let published = fx.origin_refs();
    assert_eq!(published.get("refs/heads/main"), Some(&release_head));
    assert_eq!(
        published
            .get("refs/tags/v0.1.1")
            .map(|t| fx.git_ok(&["rev-list", "-n", "1", t])),
        Some(release_head)
    );
}

#[test]
fn release_labelled_commit_with_unrelated_contents_is_not_resumed() {
    let fx = Fixture::new(true);
    // A commit carrying the release subject but also unrelated contents is not
    // a valid scoped release commit, so it must never be tagged or published.
    fx.write(
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.1.1\"\nedition = \"2021\"\n",
    );
    fx.write("unrelated.md", "swept in by an unscoped release\n");
    fx.git_ok(&["add", "Cargo.toml", "unrelated.md"]);
    fx.git_ok(&["commit", "-q", "-m", "chore(release): release v0.1.1"]);
    let bad_head = fx.head();

    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    assert!(
        stderr_of(&out).contains("not a valid scoped release commit"),
        "{}",
        describe(&out)
    );

    assert_eq!(fx.head(), bad_head, "no commit may be created");
    assert_eq!(fx.commit_count(), 2);
    assert!(
        fx.tags().is_empty(),
        "the invalid commit must not be tagged"
    );
    let published = fx.origin_refs();
    assert_ne!(published.get("refs/heads/main"), Some(&bad_head));
    assert!(!published.contains_key("refs/tags/v0.1.1"));
    assert_eq!(
        fx.manifest_version(),
        "0.1.1",
        "the version must not advance from an unresolved release state"
    );

    // Dry-run reports the same refusal without side effects.
    let dry = fx.bump(&["patch", "--dry-run"]);
    assert_failure(&dry);
    assert!(stderr_of(&dry).contains("not a valid scoped release commit"));
    assert!(fx.tags().is_empty());
}

#[test]
fn lightweight_tag_at_head_is_rejected_instead_of_published() {
    let fx = Fixture::new(true);
    let release_head = release_commit_without_tag(&fx);
    // The requirement is an annotated tag; a lightweight tag left at HEAD is
    // not acceptable evidence of a completed release.
    fx.git_ok(&["tag", "v0.1.1"]);
    assert_eq!(fx.git_ok(&["cat-file", "-t", "v0.1.1"]), "commit");

    let out = fx.bump(&["patch"]);
    assert_failure(&out);
    assert!(
        stderr_of(&out).contains("not an annotated tag"),
        "{}",
        describe(&out)
    );

    assert_eq!(fx.head(), release_head, "no commit may be created");
    assert_eq!(fx.commit_count(), 2);
    assert_eq!(
        fx.tags(),
        vec!["v0.1.1".to_string()],
        "no further version may be created"
    );
    assert_eq!(
        fx.git_ok(&["cat-file", "-t", "v0.1.1"]),
        "commit",
        "the lightweight tag must be left for the operator to resolve"
    );
    let published = fx.origin_refs();
    assert_ne!(published.get("refs/heads/main"), Some(&release_head));
    assert!(
        !published.contains_key("refs/tags/v0.1.1"),
        "an unannotated release must not be published"
    );
    assert_eq!(fx.manifest_version(), "0.1.1");

    // Recreating the tag as annotated lets the same release resume.
    fx.git_ok(&["tag", "-a", "-f", "v0.1.1", "-m", "Release v0.1.1"]);
    let resume = fx.bump(&["patch"]);
    assert_success(&resume);
    assert_eq!(fx.head(), release_head);
    assert_eq!(fx.commit_count(), 2);
    let published = fx.origin_refs();
    assert_eq!(published.get("refs/heads/main"), Some(&release_head));
    assert!(published.contains_key("refs/tags/v0.1.1"));
}

// --- Dry run ----------------------------------------------------------------

#[test]
fn dry_run_creates_no_release_side_effects() {
    let fx = Fixture::new(true);
    fx.add_unrelated_work();
    let base_head = fx.head();

    let out = fx.bump(&["patch", "--dry-run"]);
    assert_success(&out);
    assert!(stdout_of(&out).contains("[dry-run]"), "{}", describe(&out));

    assert_eq!(fx.head(), base_head);
    assert!(fx.tags().is_empty());
    assert_eq!(fx.manifest_version(), "0.1.0");
    assert_eq!(fx.origin_rev("refs/heads/main"), Some(base_head.clone()));
    fx.assert_unrelated_work_preserved();
}

#[test]
fn dry_run_in_missing_tag_recovery_state_creates_nothing() {
    let fx = Fixture::new(true);
    let release_head = release_commit_without_tag(&fx);

    let out = fx.bump(&["patch", "--dry-run"]);
    assert_success(&out);
    assert!(stdout_of(&out).contains("[dry-run]"));
    assert!(
        stdout_of(&out).contains("Would tag v0.1.1"),
        "{}",
        describe(&out)
    );

    assert_eq!(fx.head(), release_head);
    assert!(fx.tags().is_empty(), "dry-run must not create the tag");
    let published = fx.origin_refs();
    assert!(!published.contains_key("refs/tags/v0.1.1"));
    assert_ne!(published.get("refs/heads/main"), Some(&release_head));
}

#[test]
fn dry_run_in_missing_push_recovery_state_creates_nothing() {
    let fx = Fixture::new(true);
    let release_head = tagged_release_without_push(&fx);
    let origin_before = fx.origin_rev("refs/heads/main");

    let out = fx.bump(&["patch", "--dry-run"]);
    assert_success(&out);
    assert!(stdout_of(&out).contains("[dry-run]"));
    assert!(stdout_of(&out).contains("Would push"), "{}", describe(&out));

    assert_eq!(fx.head(), release_head);
    assert_eq!(fx.tags(), vec!["v0.1.1".to_string()]);
    let published = fx.origin_refs();
    assert_eq!(published.get("refs/heads/main"), origin_before.as_ref());
    assert!(!published.contains_key("refs/tags/v0.1.1"));
}

// --- Version calculation and non-main delegation ----------------------------

#[test]
fn minor_level_calculates_expected_version() {
    let fx = Fixture::new(false);
    assert_success(&fx.bump(&["minor"]));
    assert_eq!(fx.manifest_version(), "0.2.0");
    assert_eq!(fx.tags(), vec!["v0.2.0".to_string()]);
    assert_eq!(fx.head_subject(), format!("{RELEASE_COMMIT_PREFIX}0.2.0"));
}

#[test]
fn major_level_calculates_expected_version() {
    let fx = Fixture::new(false);
    assert_success(&fx.bump(&["major"]));
    assert_eq!(fx.manifest_version(), "1.0.0");
    assert_eq!(fx.tags(), vec!["v1.0.0".to_string()]);
    assert_eq!(fx.head_subject(), format!("{RELEASE_COMMIT_PREFIX}1.0.0"));
}

#[test]
fn existing_tag_for_next_version_is_skipped() {
    let fx = Fixture::new(false);
    fx.git_ok(&["tag", "-a", "v0.1.1", "-m", "Release v0.1.1"]);

    assert_success(&fx.bump(&["patch"]));

    assert_eq!(fx.manifest_version(), "0.1.2");
    assert_eq!(fx.head_subject(), format!("{RELEASE_COMMIT_PREFIX}0.1.2"));
    assert_eq!(fx.git_ok(&["rev-list", "-n", "1", "v0.1.2"]), fx.head());
}

#[test]
fn feature_branch_still_delegates_to_cargo_release_unchanged() {
    let fx = Fixture::new(true);
    fx.git_ok(&["checkout", "-q", "-b", "Feature/Nice_Branch"]);
    let base_head = fx.head();

    let out = fx.bump(&["patch"]);
    assert_success(&out);

    let log = fx.cargo_log();
    assert!(
        log.contains(
            "release 0.1.1-feature-nice-branch --allow-branch Feature/Nice_Branch \
             --no-confirm --no-publish --execute"
        ),
        "unexpected cargo invocation: {log}"
    );
    assert_eq!(fx.head(), base_head, "delegation must not commit directly");
    assert!(fx.tags().is_empty());
    assert_eq!(fx.manifest_version(), "0.1.0");
}

#[test]
fn feature_branch_dry_run_delegates_without_execute() {
    let fx = Fixture::new(false);
    fx.git_ok(&["checkout", "-q", "-b", "develop"]);

    let out = fx.bump(&["patch", "--dry-run"]);
    assert_success(&out);

    let log = fx.cargo_log();
    assert!(
        log.contains("release 0.1.1-develop --allow-branch develop --no-confirm --no-publish"),
        "unexpected cargo invocation: {log}"
    );
    assert!(
        !log.contains("--execute"),
        "dry-run must not execute: {log}"
    );
}

// --- Documented contract ----------------------------------------------------

#[test]
fn release_guide_documents_the_owned_path_contract() {
    let guide =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/guides/RELEASE.md"))
            .expect("docs/guides/RELEASE.md");

    for needle in [
        "release-owned paths",
        "`Cargo.toml`",
        "`Cargo.lock`",
        "`docs/openapi.yaml`",
        "staged, unstaged, and untracked",
        "chore(release): release vX.Y.Z",
        "--dry-run",
        "Resuming an interrupted release",
        // Ownership survives deletion of a tracked artifact, and a resume needs
        // commit/tag evidence rather than a matching subject.
        "in the index, or in",
        "Resuming only happens on evidence",
        "is annotated",
    ] {
        assert!(
            guide.contains(needle),
            "docs/guides/RELEASE.md must document {needle:?}"
        );
    }
}
