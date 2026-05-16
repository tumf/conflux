use std::collections::VecDeque;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use chrono::Local;
use thiserror::Error;

use crate::config::defaults;

/// Default number of lines printed by `cflx logs` when no explicit mode is selected.
pub const DEFAULT_TAIL_LINES: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogSelection {
    pub log_root: PathBuf,
    pub project_slug: String,
    pub project_dir: PathBuf,
    pub selected_file: PathBuf,
    pub available_projects: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogViewerOptions {
    pub print_path: bool,
    pub last: Option<usize>,
    pub follow: bool,
    pub today: bool,
    pub project: Option<String>,
    pub repo_root: Option<PathBuf>,
}

#[derive(Error, Debug)]
pub enum LogViewerError {
    #[error("{0}")]
    Actionable(ActionableLogError),

    #[error("IO error while viewing logs at '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionableLogError {
    pub message: String,
    pub log_root: PathBuf,
    pub available_projects: Vec<String>,
}

impl fmt::Display for ActionableLogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.message)?;
        writeln!(f, "log root: {}", self.log_root.display())?;
        if self.available_projects.is_empty() {
            write!(f, "available projects: none")
        } else {
            writeln!(f, "available projects:")?;
            for project in &self.available_projects {
                writeln!(f, "  - {project}")?;
            }
            write!(f, "Use --project <slug> to select one of these projects.")
        }
    }
}

pub type LogViewerResult<T> = std::result::Result<T, LogViewerError>;

pub fn run_logs_command<W: Write>(
    options: &LogViewerOptions,
    writer: &mut W,
) -> LogViewerResult<()> {
    tracing::debug!(
        module = module_path!(),
        print_path = options.print_path,
        last = ?options.last,
        follow = options.follow,
        today = options.today,
        project = ?options.project,
        repo_root = ?options.repo_root,
        "Resolving log viewer selection"
    );

    let selection = resolve_log_selection(options)?;

    tracing::debug!(
        module = module_path!(),
        project_slug = %selection.project_slug,
        selected_file = %selection.selected_file.display(),
        "Resolved log viewer selection"
    );

    if options.print_path {
        writeln!(writer, "{}", selection.selected_file.display()).map_err(|source| {
            LogViewerError::Io {
                path: selection.selected_file.clone(),
                source,
            }
        })?;
        return Ok(());
    }

    let line_count = options.last.unwrap_or(DEFAULT_TAIL_LINES);
    write_last_lines(&selection.selected_file, line_count, writer)?;

    if options.follow {
        follow_appended_lines(
            &selection.selected_file,
            writer,
            Duration::from_millis(250),
            None,
        )?;
    }

    Ok(())
}

pub fn resolve_log_selection(options: &LogViewerOptions) -> LogViewerResult<LogSelection> {
    let log_root = log_root_path();
    let available_projects = list_available_projects_in(&log_root)?;
    let project_slug = match &options.project {
        Some(project) => project.clone(),
        None => options
            .repo_root
            .as_deref()
            .map(defaults::generate_project_slug)
            .unwrap_or_else(|| "unknown".to_string()),
    };
    let project_dir = log_root.join(&project_slug);

    let selected_file = if options.today {
        today_log_path_in_project(&project_dir)
    } else if let Some(latest) = latest_log_file_in_project(&project_dir)? {
        latest
    } else {
        today_log_path_in_project(&project_dir)
    };

    if !options.print_path && !selected_file.is_file() {
        return Err(LogViewerError::Actionable(ActionableLogError {
            message: format!(
                "No Conflux log file found for project '{project_slug}' at '{}'.",
                selected_file.display()
            ),
            log_root,
            available_projects,
        }));
    }

    if options.project.is_some() && !options.print_path && !project_dir.is_dir() {
        return Err(LogViewerError::Actionable(ActionableLogError {
            message: format!(
                "No Conflux log project directory found for explicit project '{project_slug}'."
            ),
            log_root,
            available_projects,
        }));
    }

    Ok(LogSelection {
        log_root,
        project_slug,
        project_dir,
        selected_file,
        available_projects,
    })
}

pub fn log_root_path() -> PathBuf {
    if let Ok(xdg_state_home) = std::env::var("XDG_STATE_HOME") {
        if !xdg_state_home.is_empty() {
            return PathBuf::from(xdg_state_home).join("cflx").join("logs");
        }
    }

    if let Some(home) = dirs::home_dir() {
        return home.join(".local").join("state").join("cflx").join("logs");
    }

    std::env::temp_dir().join("cflx-logs-fallback")
}

pub fn list_available_projects_in(log_root: &Path) -> LogViewerResult<Vec<String>> {
    if !log_root.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(log_root).map_err(|source| LogViewerError::Io {
        path: log_root.to_path_buf(),
        source,
    })?;

    let mut projects = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| LogViewerError::Io {
            path: log_root.to_path_buf(),
            source,
        })?;
        let file_type = entry.file_type().map_err(|source| LogViewerError::Io {
            path: entry.path(),
            source,
        })?;
        if file_type.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                projects.push(name.to_string());
            }
        }
    }
    projects.sort();
    Ok(projects)
}

pub fn latest_log_file_in_project(project_dir: &Path) -> LogViewerResult<Option<PathBuf>> {
    if !project_dir.exists() {
        return Ok(None);
    }

    let entries = fs::read_dir(project_dir).map_err(|source| LogViewerError::Io {
        path: project_dir.to_path_buf(),
        source,
    })?;

    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| LogViewerError::Io {
            path: project_dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("log") {
            candidates.push(path);
        }
    }

    candidates.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(candidates.pop())
}

pub fn today_log_path_in_project(project_dir: &Path) -> PathBuf {
    project_dir.join(format!("{}.log", Local::now().format("%Y-%m-%d")))
}

pub fn read_last_lines(path: &Path, line_count: usize) -> LogViewerResult<Vec<String>> {
    if line_count == 0 {
        return Ok(Vec::new());
    }

    let file = File::open(path).map_err(|source| LogViewerError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = BufReader::new(file);
    let mut lines = VecDeque::with_capacity(line_count.min(1024));

    for line in reader.lines() {
        let line = line.map_err(|source| LogViewerError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if lines.len() == line_count {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    Ok(lines.into_iter().collect())
}

pub fn write_last_lines<W: Write>(
    path: &Path,
    line_count: usize,
    writer: &mut W,
) -> LogViewerResult<()> {
    for line in read_last_lines(path, line_count)? {
        writeln!(writer, "{line}").map_err(|source| LogViewerError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

pub fn follow_appended_lines<W: Write>(
    path: &Path,
    writer: &mut W,
    poll_interval: Duration,
    stop_after_lines: Option<usize>,
) -> LogViewerResult<()> {
    let mut file = File::open(path).map_err(|source| LogViewerError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    file.seek(SeekFrom::End(0))
        .map_err(|source| LogViewerError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let mut reader = BufReader::new(file);
    let mut emitted = 0usize;

    loop {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|source| LogViewerError::Io {
                path: path.to_path_buf(),
                source,
            })?;

        if bytes == 0 {
            if stop_after_lines == Some(emitted) {
                return Ok(());
            }
            thread::sleep(poll_interval);
            continue;
        }

        writer
            .write_all(line.as_bytes())
            .and_then(|_| writer.flush())
            .map_err(|source| LogViewerError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        emitted += 1;

        if stop_after_lines == Some(emitted) {
            return Ok(());
        }
    }
}

/// Read the entire file into a string without mutating it; used by tests to assert
/// read-only behavior.
#[cfg(test)]
pub fn read_file_for_size_check(path: &Path) -> io::Result<String> {
    let mut content = String::new();
    File::open(path)?.read_to_string(&mut content)?;
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn resolves_explicit_project_and_latest_log() {
        let tmp = tempfile::tempdir().unwrap();
        let log_root = tmp.path().join("cflx/logs");
        let project_dir = log_root.join("alpha");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("2026-01-01.log"), "old\n").unwrap();
        fs::write(project_dir.join("2026-01-03.log"), "new\n").unwrap();

        let projects = list_available_projects_in(&log_root).unwrap();
        assert_eq!(projects, vec!["alpha"]);
        assert_eq!(
            latest_log_file_in_project(&project_dir).unwrap(),
            Some(project_dir.join("2026-01-03.log"))
        );
    }

    #[test]
    fn tail_reads_at_most_requested_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let log_file = tmp.path().join("test.log");
        fs::write(&log_file, "a\nb\nc\nd\n").unwrap();

        assert_eq!(read_last_lines(&log_file, 2).unwrap(), vec!["c", "d"]);
        assert!(read_last_lines(&log_file, 0).unwrap().is_empty());
    }

    #[test]
    fn missing_file_error_lists_available_projects() {
        let tmp = tempfile::tempdir().unwrap();
        let log_root = tmp.path().join("cflx/logs");
        fs::create_dir_all(log_root.join("alpha")).unwrap();
        fs::create_dir_all(log_root.join("beta")).unwrap();

        let err = ActionableLogError {
            message: "No Conflux log file found".to_string(),
            log_root: log_root.clone(),
            available_projects: list_available_projects_in(&log_root).unwrap(),
        }
        .to_string();

        assert!(err.contains("alpha"));
        assert!(err.contains("beta"));
        assert!(err.contains("--project <slug>"));
    }

    #[test]
    fn follow_streams_appended_lines_without_truncating_file() {
        let tmp = tempfile::tempdir().unwrap();
        let log_file = tmp.path().join("follow.log");
        fs::write(&log_file, "existing\n").unwrap();
        let before = fs::metadata(&log_file).unwrap().len();
        let path_for_thread = log_file.clone();
        let (tx, rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let mut output = Vec::new();
            follow_appended_lines(
                &path_for_thread,
                &mut output,
                Duration::from_millis(10),
                Some(2),
            )
            .unwrap();
            tx.send(String::from_utf8(output).unwrap()).unwrap();
        });

        thread::sleep(Duration::from_millis(50));
        let mut file = fs::OpenOptions::new().append(true).open(&log_file).unwrap();
        writeln!(file, "new-1").unwrap();
        writeln!(file, "new-2").unwrap();
        drop(file);

        handle.join().unwrap();
        assert_eq!(rx.recv().unwrap(), "new-1\nnew-2\n");
        let after = fs::metadata(&log_file).unwrap().len();
        assert!(after > before);
        assert_eq!(
            read_file_for_size_check(&log_file).unwrap(),
            "existing\nnew-1\nnew-2\n"
        );
    }
}
